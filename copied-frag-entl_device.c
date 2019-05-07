
/**
 * entl_e1000e_set_rx_mode - ENTL versin, always set Promiscuous mode
 * @netdev: network interface device structure
 *
 * The ndo_set_rx_mode entry point is called whenever the unicast or multicast
 * address list or the network interface flags are updated.  This routine is
 * responsible for configuring the hardware for proper unicast, multicast,
 * promiscuous mode, and all-multi behavior.
 **/
static void entl_e1000e_set_rx_mode(struct net_device *netdev)
{
	struct e1000_adapter *adapter = netdev_priv(netdev);
	struct e1000_hw *hw = &adapter->hw;
	u32 rctl;

	if (pm_runtime_suspended(netdev->dev.parent))
		return;

	/* Check for Promiscuous and All Multicast modes */
	rctl = er32(RCTL);                                           

#ifdef ENTL
	// behave as if IFF_PROMISC is always set
	rctl |= (E1000_RCTL_UPE | E1000_RCTL_MPE);
#ifdef HAVE_VLAN_RX_REGISTER
	rctl &= ~E1000_RCTL_VFE;
#else
	/* Do not hardware filter VLANs in promisc mode */
	e1000e_vlan_filter_disable(adapter);
#endif /* HAVE_VLAN_RX_REGISTER */

    ENTL_DEBUG("entl_e1000e_set_rx_mode  RCTL = %08x\n", rctl );
#else
	/* clear the affected bits */
	rctl &= ~(E1000_RCTL_UPE | E1000_RCTL_MPE);

	if (netdev->flags & IFF_PROMISC) {
		rctl |= (E1000_RCTL_UPE | E1000_RCTL_MPE);
#ifdef HAVE_VLAN_RX_REGISTER
		rctl &= ~E1000_RCTL_VFE;
#else
		/* Do not hardware filter VLANs in promisc mode */
		e1000e_vlan_filter_disable(adapter);
#endif /* HAVE_VLAN_RX_REGISTER */
	} else {
		int count;

		if (netdev->flags & IFF_ALLMULTI) {
			rctl |= E1000_RCTL_MPE;
		} else {
			/* Write addresses to the MTA, if the attempt fails
			 * then we should just turn on promiscuous mode so
			 * that we can at least receive multicast traffic
			 */
			count = e1000e_write_mc_addr_list(netdev);
			if (count < 0)
				rctl |= E1000_RCTL_MPE;
		}
#ifdef HAVE_VLAN_RX_REGISTER
		if (adapter->flags & FLAG_HAS_HW_VLAN_FILTER)
			rctl |= E1000_RCTL_VFE;
#else
		e1000e_vlan_filter_enable(adapter);
#endif
#ifdef HAVE_SET_RX_MODE
		/* Write addresses to available RAR registers, if there is not
		 * sufficient space to store all the addresses then enable
		 * unicast promiscuous mode
		 */
		count = e1000e_write_uc_addr_list(netdev);
		if (count < 0)
			rctl |= E1000_RCTL_UPE;
#endif /* HAVE_SET_RX_MODE */
	}
#endif /* ENTL */

	ew32(RCTL, rctl);
#ifndef HAVE_VLAN_RX_REGISTER

#ifdef NETIF_F_HW_VLAN_CTAG_RX
	if (netdev->features & NETIF_F_HW_VLAN_CTAG_RX)
#else
	if (netdev->features & NETIF_F_HW_VLAN_RX)
#endif
		e1000e_vlan_strip_enable(adapter);
	else
		e1000e_vlan_strip_disable(adapter);
#endif /* HAVE_VLAN_RX_REGISTER */
}

/**
 * entl_e1000_setup_rctl - ENTL version of configure the receive control registers
 * @adapter: Board private structure
 **/
static void entl_e1000_setup_rctl(struct e1000_adapter *adapter)
{
	struct e1000_hw *hw = &adapter->hw;
	u32 rctl, rfctl;
	u32 pages = 0;

	/* Workaround Si errata on PCHx - configure jumbo frame flow.
	 * If jumbo frames not set, program related MAC/PHY registers
	 * to h/w defaults
	 */
	if (hw->mac.type >= e1000_pch2lan) {
		s32 ret_val;

		if (adapter->netdev->mtu > ETH_DATA_LEN)
			ret_val = e1000_lv_jumbo_workaround_ich8lan(hw, true);
		else
			ret_val = e1000_lv_jumbo_workaround_ich8lan(hw, false);

		if (ret_val)
			e_dbg("failed to enable|disable jumbo frame workaround mode\n");
	}

	/* Program MC offset vector base */
	rctl = er32(RCTL);
	rctl &= ~(3 << E1000_RCTL_MO_SHIFT);
	rctl |= E1000_RCTL_EN | E1000_RCTL_BAM |
	    E1000_RCTL_LBM_NO | E1000_RCTL_RDMTS_HALF |
	    (adapter->hw.mac.mc_filter_type << E1000_RCTL_MO_SHIFT);

	/* Do not Store bad packets */
	rctl &= ~E1000_RCTL_SBP;

	/* Enable Long Packet receive */
	if (adapter->netdev->mtu <= ETH_DATA_LEN) {
		ENTL_DEBUG("entl_e1000_setup_rctl %d <= %d\n", adapter->netdev->mtu, ETH_DATA_LEN );
		rctl &= ~E1000_RCTL_LPE;
	}
	else {
		ENTL_DEBUG("entl_e1000_setup_rctl %d > %d\n", adapter->netdev->mtu, ETH_DATA_LEN );
		rctl |= E1000_RCTL_LPE;
	}

	/* Some systems expect that the CRC is included in SMBUS traffic. The
	 * hardware strips the CRC before sending to both SMBUS (BMC) and to
	 * host memory when this is enabled
	 */
	if (adapter->flags2 & FLAG2_CRC_STRIPPING)
		rctl |= E1000_RCTL_SECRC;

	/* Workaround Si errata on 82577/82578 - configure IPG for jumbos */
	if ((hw->mac.type == e1000_pchlan) && (rctl & E1000_RCTL_LPE)) {
		u32 mac_data;
		u16 phy_data;

		ENTL_DEBUG("entl_e1000_setup_rctl Workaround Si errata on 82577/82578 - configure IPG for jumbos\n" );

		e1e_rphy(hw, PHY_REG(770, 26), &phy_data);
		phy_data &= 0xfff8;
		phy_data |= (1 << 2);
		e1e_wphy(hw, PHY_REG(770, 26), phy_data);

		mac_data = er32(FFLT_DBG);
		mac_data |= (1 << 17);
		ew32(FFLT_DBG, mac_data);

		if (hw->phy.type == e1000_phy_82577) {
			e1e_rphy(hw, 22, &phy_data);
			phy_data &= 0x0fff;
			phy_data |= (1 << 14);
			e1e_wphy(hw, 0x10, 0x2823);
			e1e_wphy(hw, 0x11, 0x0003);
			e1e_wphy(hw, 22, phy_data);
		}
	}

	/* Setup buffer sizes */
	rctl &= ~E1000_RCTL_SZ_4096;
	rctl |= E1000_RCTL_BSEX;
	switch (adapter->rx_buffer_len) {
	case 2048:
	default:
		ENTL_DEBUG("entl_e1000_setup_rctl E1000_RCTL_SZ_2048\n" );
		rctl |= E1000_RCTL_SZ_2048;
		rctl &= ~E1000_RCTL_BSEX;
		break;
	case 4096:
		ENTL_DEBUG("entl_e1000_setup_rctl E1000_RCTL_SZ_4096\n" );
		rctl |= E1000_RCTL_SZ_4096;
		break;
	case 8192:
		ENTL_DEBUG("entl_e1000_setup_rctl E1000_RCTL_SZ_8192\n" );
		rctl |= E1000_RCTL_SZ_8192;
		break;
	case 16384:
		ENTL_DEBUG("entl_e1000_setup_rctl E1000_RCTL_SZ_16384\n" );
		rctl |= E1000_RCTL_SZ_16384;
		break;
	}

	/* Enable Extended Status in all Receive Descriptors */
	rfctl = er32(RFCTL);
	rfctl |= E1000_RFCTL_EXTEN;
	ew32(RFCTL, rfctl);

	/* 82571 and greater support packet-split where the protocol
	 * header is placed in skb->data and the packet data is
	 * placed in pages hanging off of skb_shinfo(skb)->nr_frags.
	 * In the case of a non-split, skb->data is linearly filled,
	 * followed by the page buffers.  Therefore, skb->data is
	 * sized to hold the largest protocol header.
	 *
	 * allocations using alloc_page take too long for regular MTU
	 * so only enable packet split for jumbo frames
	 *
	 * Using pages when the page size is greater than 16k wastes
	 * a lot of memory, since we allocate 3 pages at all times
	 * per packet.
	 */
	pages = PAGE_USE_COUNT(adapter->netdev->mtu);
	if ((pages <= 3) && (PAGE_SIZE <= 16384) && (rctl & E1000_RCTL_LPE))
		adapter->rx_ps_pages = pages;
	else
		adapter->rx_ps_pages = 0;

	ENTL_DEBUG("entl_e1000_setup_rctl rx_ps_pages = %d\n", adapter->rx_ps_pages );

	if (adapter->rx_ps_pages) {
		u32 psrctl = 0;

		/* Enable Packet split descriptors */
		rctl |= E1000_RCTL_DTYP_PS;

		psrctl |= adapter->rx_ps_bsize0 >> E1000_PSRCTL_BSIZE0_SHIFT;

		switch (adapter->rx_ps_pages) {
		case 3:
			psrctl |= PAGE_SIZE << E1000_PSRCTL_BSIZE3_SHIFT;
			/* fall-through */
		case 2:
			psrctl |= PAGE_SIZE << E1000_PSRCTL_BSIZE2_SHIFT;
			/* fall-through */
		case 1:
			psrctl |= PAGE_SIZE >> E1000_PSRCTL_BSIZE1_SHIFT;
			break;
		}

		ew32(PSRCTL, psrctl);
	}

	/* This is useful for sniffing bad packets. */
	if (adapter->netdev->features & NETIF_F_RXALL) {
		/* UPE and MPE will be handled by normal PROMISC logic
		 * in e1000e_set_rx_mode
		 */
		rctl |= (E1000_RCTL_SBP |	/* Receive bad packets */
			 E1000_RCTL_BAM |	/* RX All Bcast Pkts */
			 E1000_RCTL_PMCF);	/* RX All MAC Ctrl Pkts */

		rctl &= ~(E1000_RCTL_VFE |	/* Disable VLAN filter */
			  E1000_RCTL_DPF |	/* Allow filtered pause */
			  E1000_RCTL_CFIEN);	/* Dis VLAN CFIEN Filter */
		/* Do not mess with E1000_CTRL_VME, it affects transmit as well,
		 * and that breaks VLANs.
		 */
	}
    ENTL_DEBUG("entl_e1000_setup_rctl  RCTL = %08x\n", rctl );

	ew32(RCTL, rctl);
	/* just started the receive unit, no need to restart */
	adapter->flags &= ~FLAG_RESTART_NOW;
}

/**
 * entl_e1000_configure_rx - ENTL version of Configure Receive Unit after Reset
 * @adapter: board private structure
 *
 * Configure the Rx unit of the MAC after a reset.
 **/
static void entl_e1000_configure_rx(struct e1000_adapter *adapter)
{
	struct e1000_hw *hw = &adapter->hw;
	struct e1000_ring *rx_ring = adapter->rx_ring;
	u64 rdba;
	u32 rdlen, rctl, rxcsum, ctrl_ext;

	if (adapter->rx_ps_pages) {
		/* this is a 32 byte descriptor */
		rdlen = rx_ring->count *
		    sizeof(union e1000_rx_desc_packet_split);
		adapter->clean_rx = e1000_clean_rx_irq_ps;
		adapter->alloc_rx_buf = e1000_alloc_rx_buffers_ps;
		ENTL_DEBUG("entl_e1000_configure_rx use e1000_alloc_rx_buffers_ps\n" );
#ifdef CONFIG_E1000E_NAPI
	} else if (adapter->netdev->mtu > ETH_FRAME_LEN + ETH_FCS_LEN) {
		rdlen = rx_ring->count * sizeof(union e1000_rx_desc_extended);
		adapter->clean_rx = e1000_clean_jumbo_rx_irq;
		adapter->alloc_rx_buf = e1000_alloc_jumbo_rx_buffers;
		ENTL_DEBUG("entl_e1000_configure_rx use e1000_alloc_jumbo_rx_buffers\n" );
#endif
	} else {
		rdlen = rx_ring->count * sizeof(union e1000_rx_desc_extended);
		adapter->clean_rx = e1000_clean_rx_irq;
		adapter->alloc_rx_buf = e1000_alloc_rx_buffers;
		ENTL_DEBUG("entl_e1000_configure_rx use e1000_alloc_rx_buffers\n" );
	}

	/* disable receives while setting up the descriptors */
	rctl = er32(RCTL);
	if (!(adapter->flags2 & FLAG2_NO_DISABLE_RX))
		ew32(RCTL, rctl & ~E1000_RCTL_EN);
	e1e_flush();
	usleep_range(10000, 20000);

	if (adapter->flags2 & FLAG2_DMA_BURST) {
		ENTL_DEBUG("entl_e1000_configure_rx set DMA burst\n" );
		/* set the writeback threshold (only takes effect if the RDTR
		 * is set). set GRAN=1 and write back up to 0x4 worth, and
		 * enable prefetching of 0x20 Rx descriptors
		 * granularity = 01
		 * wthresh = 04,
		 * hthresh = 04,
		 * pthresh = 0x20
		 */
		ew32(RXDCTL(0), E1000_RXDCTL_DMA_BURST_ENABLE);
		ew32(RXDCTL(1), E1000_RXDCTL_DMA_BURST_ENABLE);

		/* override the delay timers for enabling bursting, only if
		 * the value was not set by the user via module options
		 */
		if (adapter->rx_int_delay == DEFAULT_RDTR)
			adapter->rx_int_delay = BURST_RDTR;
		if (adapter->rx_abs_int_delay == DEFAULT_RADV)
			adapter->rx_abs_int_delay = BURST_RADV;
	}

	/* set the Receive Delay Timer Register */
	ENTL_DEBUG("entl_e1000_configure_rx set Receive Delay Timer Register = %d\n", adapter->rx_int_delay );
	ew32(RDTR, adapter->rx_int_delay);

	/* irq moderation */
	ENTL_DEBUG("entl_e1000_configure_rx set Abs Delay Timer Register = %d\n", adapter->rx_abs_int_delay );
	ew32(RADV, adapter->rx_abs_int_delay);
	if ((adapter->itr_setting != 0) && (adapter->itr != 0))
		e1000e_write_itr(adapter, adapter->itr);

	ctrl_ext = er32(CTRL_EXT);
#ifdef CONFIG_E1000E_NAPI
	/* Auto-Mask interrupts upon ICR access */
	ctrl_ext |= E1000_CTRL_EXT_IAME;
	ew32(IAM, 0xffffffff);
#endif
	ew32(CTRL_EXT, ctrl_ext);
	e1e_flush();

	/* Setup the HW Rx Head and Tail Descriptor Pointers and
	 * the Base and Length of the Rx Descriptor Ring
	 */
	rdba = rx_ring->dma;
	ew32(RDBAL(0), (rdba & DMA_BIT_MASK(32)));
	ew32(RDBAH(0), (rdba >> 32));
	ew32(RDLEN(0), rdlen);
	ew32(RDH(0), 0);
	ew32(RDT(0), 0);
	rx_ring->head = adapter->hw.hw_addr + E1000_RDH(0);
	rx_ring->tail = adapter->hw.hw_addr + E1000_RDT(0);

	/* Enable Receive Checksum Offload for TCP and UDP */
	rxcsum = er32(RXCSUM);
#ifdef HAVE_NDO_SET_FEATURES
	if (adapter->netdev->features & NETIF_F_RXCSUM)
#else
	if (adapter->flags & FLAG_RX_CSUM_ENABLED)
#endif
		rxcsum |= E1000_RXCSUM_TUOFL;
	else
		rxcsum &= ~E1000_RXCSUM_TUOFL;
	ew32(RXCSUM, rxcsum);

	/* With jumbo frames, excessive C-state transition latencies result
	 * in dropped transactions.
	 */
	if (adapter->netdev->mtu > ETH_DATA_LEN) {
		u32 lat =
		    ((er32(PBA) & E1000_PBA_RXA_MASK) * 1024 -
		     adapter->max_frame_size) * 8 / 1000;

		ENTL_DEBUG("entl_e1000_configure_rx adapter->netdev->mtu %d > ETH_DATA_LEN %d lat = %d\n", adapter->netdev->mtu, ETH_DATA_LEN, lat );

		if (adapter->flags & FLAG_IS_ICH) {
			u32 rxdctl = er32(RXDCTL(0));

			ew32(RXDCTL(0), rxdctl | 0x3);
		}
#ifdef HAVE_PM_QOS_REQUEST_LIST_NEW
		pm_qos_update_request(&adapter->pm_qos_req, lat);
#elif defined(HAVE_PM_QOS_REQUEST_LIST)
		pm_qos_update_request(&adapter->pm_qos_req, lat);
#else
		pm_qos_update_requirement(PM_QOS_CPU_DMA_LATENCY,
					  adapter->netdev->name, lat);
#endif
	} else {
		ENTL_DEBUG("entl_e1000_configure_rx adapter->netdev->mtu %d <= ETH_DATA_LEN %d default qos = %d\n", adapter->netdev->mtu, ETH_DATA_LEN, PM_QOS_DEFAULT_VALUE );

#ifdef HAVE_PM_QOS_REQUEST_LIST_NEW
		pm_qos_update_request(&adapter->pm_qos_req,
				      PM_QOS_DEFAULT_VALUE);
#elif defined(HAVE_PM_QOS_REQUEST_LIST)
		pm_qos_update_request(&adapter->pm_qos_req,
				      PM_QOS_DEFAULT_VALUE);
#else
		pm_qos_update_requirement(PM_QOS_CPU_DMA_LATENCY,
					  adapter->netdev->name,
					  PM_QOS_DEFAULT_VALUE);
#endif
	}
	ENTL_DEBUG("entl_e1000_configure_rx  RCTL = %08x\n", rctl );

	/* Enable Receives */
	ew32(RCTL, rctl);
}
