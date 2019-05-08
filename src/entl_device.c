
#define FETCH_STATE(stm) 0 /* ((stm)->current_state.current_state) */
#define ENTL_DEBUG(fmt, args...) printk(KERN_ALERT "ENTL:" fmt, ## args)

#include "entl_skb_queue.h"
#include "entl_state_machine.h"

// entl_state_machine entry points:
extern void entl_link_up(entl_state_machine_t *mcn);
extern int entl_next_send(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr); // ENTL_ACTION
extern int entl_next_send_tx(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr); // ENTL_ACTION
extern int entl_received(entl_state_machine_t *mcn, uint16_t u_saddr, uint32_t l_saddr, uint16_t u_daddr, uint32_t l_daddr); // ENTL_ACTION
extern void entl_state_error(entl_state_machine_t *mcn, uint32_t error_flag); // enter error state
#include "entl_ioctl.h"
extern void entl_new_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data);
extern int entl_send_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data);
extern struct entt_ioctl_ait_data *entl_next_AIT_message(entl_state_machine_t *mcn);
extern struct entt_ioctl_ait_data *entl_read_AIT_message(entl_state_machine_t *mcn); 

extern void entl_read_current_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err);
extern void entl_read_error_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err);

extern uint16_t entl_num_queued(entl_state_machine_t *mcn);

extern void entl_state_machine_init(entl_state_machine_t *mcn);
extern void entl_set_my_adder(entl_state_machine_t *mcn, uint16_t u_addr, uint32_t l_addr); 

// algorithm:
extern int entl_get_hello(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr);

#include "entl_device.h"

// copied e1000 routines:
static void entl_e1000e_set_rx_mode(struct net_device *netdev);
static void entl_e1000_setup_rctl(struct e1000_adapter *adapter);
static void entl_e1000_configure_rx(struct e1000_adapter *adapter);

// back references to netdev.c
static netdev_tx_t e1000_xmit_frame(struct sk_buff *skb, struct net_device *netdev);

// needed by netdev.c
static void entl_device_init(entl_device_t *dev);
static void entl_device_link_down(entl_device_t *dev);
static void entl_device_link_up(entl_device_t *dev);
static bool entl_device_process_rx_packet(entl_device_t *dev, struct sk_buff *skb);
static void entl_device_process_tx_packet(entl_device_t *dev, struct sk_buff *skb);
static int entl_do_ioctl(struct net_device *netdev, struct ifreq *ifr, int cmd);
static void entl_e1000_configure(struct e1000_adapter *adapter);
static void entl_e1000_set_my_addr(entl_device_t *dev, const uint8_t *addr);
#ifdef ENTL_TX_ON_ENTL_ENABLE
static netdev_tx_t entl_tx_transmit(struct sk_buff *skb, struct net_device *netdev);
#endif

// forward declarations (internal/private)
static int inject_message(entl_device_t *dev, uint16_t u_addr, uint32_t l_addr, int flag);
static void entl_watchdog(unsigned long data);
static void entl_watchdog_task(struct work_struct *work);
// static void dump_state(char *type, entl_state_t *st, int flag); // debug

// netdev entry points:
static void entl_device_init(entl_device_t *dev) {
    memset(dev, 0, sizeof(struct entl_device));
    // watchdog timer & task setup
    init_timer(&dev->edev_watchdog_timer);
    dev->edev_watchdog_timer.function = entl_watchdog;
    dev->edev_watchdog_timer.data = (unsigned long) dev;
    INIT_WORK(&dev->edev_watchdog_task, entl_watchdog_task);
    ENTL_skb_queue_init(&dev->edev_tx_skb_queue);
    dev->edev_queue_stopped = 0;
}

static void entl_device_link_down(entl_device_t *dev) {
    struct entl_state_machine *stm = &dev->edev_stm;
    entl_state_error(stm, ENTL_ERROR_FLAG_LINKDONW);
    dev->edev_flag = ENTL_DEVICE_FLAG_SIGNAL;
    mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
}

static void entl_device_link_up(entl_device_t *dev) {
    struct entl_state_machine *stm = &dev->edev_stm;
    entl_link_up(stm);
    dev->edev_flag |= ENTL_DEVICE_FLAG_SIGNAL;
    mod_timer(&dev->edev_watchdog_timer, jiffies + 1);

    // FIXME: why redundant watchdog ??
    uint32_t entl_state = FETCH_STATE(stm);
    if (entl_state == ENTL_STATE_HELLO) {
        dev->edev_flag |= ENTL_DEVICE_FLAG_HELLO;
        mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
    }
}

// ISR context
// returns
// true when netdev.c should continue to process packet
// false when packet has been consumed
static bool entl_device_process_rx_packet(entl_device_t *dev, struct sk_buff *skb) {
    struct e1000_adapter *adapter = container_of(dev, struct e1000_adapter, entl_dev);
    struct ethhdr *eth = (struct ethhdr *) skb->data;

    uint16_t src_u = (uint16_t) eth->h_source[0] << 8
                             |  eth->h_source[1];
    uint32_t src_l = (uint32_t) eth->h_source[2] << 24
                   | (uint32_t) eth->h_source[3] << 16
                   | (uint32_t) eth->h_source[4] <<  8
                   | (uint32_t) eth->h_source[5];

    uint16_t dst_u = (uint16_t) eth->h_dest[0] << 8
                              | eth->h_dest[1];
    uint32_t dst_l = (uint32_t) eth->h_dest[2] << 24
                   | (uint32_t) eth->h_dest[3] << 16
                   | (uint32_t) eth->h_dest[4] <<  8
                   | (uint32_t) eth->h_dest[5];

    bool retval = true;
    if (dst_u & ENTL_MESSAGE_ONLY_U) retval = false;

    struct entl_state_machine *stm = &dev->edev_stm;
    int result = entl_received(stm, src_u, src_l, dst_u, dst_l);
    if (result == ENTL_ACTION_ERROR) {
        dev->edev_flag |= (ENTL_DEVICE_FLAG_HELLO | ENTL_DEVICE_FLAG_SIGNAL);
        mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
        return retval;
    }

    if (result == ENTL_ACTION_SIG_ERR) {
        dev->edev_flag |= ENTL_DEVICE_FLAG_SIGNAL;
        mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
        return retval;
    }

    if (result & ENTL_ACTION_PROC_AIT) {
        unsigned int len = skb->len;
        if (len > sizeof(struct ethhdr)) {
            struct entt_ioctl_ait_data *ait_data = kzalloc(sizeof(struct entt_ioctl_ait_data), GFP_ATOMIC);
            unsigned char *data = skb->data + sizeof(struct ethhdr);
            uint32_t nbytes;
            memcpy(&nbytes, data, sizeof(uint32_t));
            // FIXME: MAX_AIT_MESSAGE_SIZE 256
            if ((nbytes > 0) && (nbytes < MAX_AIT_MESSAGE_SIZE)) {
                unsigned char *payload = data + sizeof(uint32_t);
                ait_data->message_len = nbytes;
                memcpy(ait_data->data, payload, nbytes);
                // ait_data->num_messages = 0;
                // ait_data->num_queued = 0;
            }
            else {
                ait_data->message_len = 0;
                // ait_data->num_messages = 0;
                // ait_data->num_queued = 0;
            }
            entl_new_AIT_message(stm, ait_data);
        }
        else {
            // FIXME
        }
    }

    if (result & ENTL_ACTION_SIG_AIT) {
        dev->edev_flag |= ENTL_DEVICE_FLAG_SIGNAL2;
    }

    if (result & ENTL_ACTION_SEND) {
        // SEND_DAT flag is set on SEND state to check if TX queue has data
        if (result & ENTL_ACTION_SEND_DAT && ENTL_skb_queue_has_data(&dev->edev_tx_skb_queue)) {
            // TX queue has data, so transfer with data
            struct sk_buff *dt = ENTL_skb_queue_front_pop(&dev->edev_tx_skb_queue);
            while (NULL != dt && skb_is_gso(dt)) { // GSO can't be used for ENTL
                e1000_xmit_frame(dt, adapter->netdev);
                dt = ENTL_skb_queue_front_pop(&dev->edev_tx_skb_queue);
            }

            if (dt) {
                e1000_xmit_frame(dt, adapter->netdev);
            }
            else {
                // tx queue becomes empty, so inject a new packet
                int ret = entl_next_send(stm, &dst_u, &dst_l);
                if ((dst_u & (uint16_t) ENTL_MESSAGE_MASK) != ENTL_MESSAGE_NOP_U) {
                    unsigned long flags;
                    spin_lock_irqsave(&adapter->entl_txring_lock, flags);
                    result = inject_message(dev, dst_u, dst_l, ret);
                    spin_unlock_irqrestore(&adapter->entl_txring_lock, flags);

                    // failed inject, invoke task
                    if (result == 1) {
                        // resource error, retry
                        dev->edev_u_addr = dst_u;
                        dev->edev_l_addr = dst_l;
                        dev->edev_action = ret;
                        dev->edev_flag |= ENTL_DEVICE_FLAG_RETRY;
                        mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
                    }
                    else if (result == -1) {
                        entl_state_error(stm, ENTL_ERROR_FATAL);
                        dev->edev_flag |= ENTL_DEVICE_FLAG_SIGNAL;
                        mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
                    }
                    else {
                        dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_WAITING;
                    }
                }
                else {
                    // ENTL_MESSAGE_NOP_U
                }
            }

            // netif queue handling for flow control
            if (dev->edev_queue_stopped && ENTL_skb_queue_unused(&dev->edev_tx_skb_queue) > 2) {
                netif_start_queue(adapter->netdev);
                dev->edev_queue_stopped = 0;
            }
        }
        else {
            int ret = entl_next_send(stm, &dst_u, &dst_l);
            if ((dst_u & (uint16_t) ENTL_MESSAGE_MASK) != ENTL_MESSAGE_NOP_U) {
                unsigned long flags;
                spin_lock_irqsave(&adapter->entl_txring_lock, flags);
                result = inject_message(dev, dst_u, dst_l, ret);
                spin_unlock_irqrestore(&adapter->entl_txring_lock, flags);
                // failed inject, invoke task
                if (result == 1) {
                    // resource error, so retry
                    dev->edev_u_addr = dst_u;
                    dev->edev_l_addr = dst_l;
                    dev->edev_action = ret;
                    dev->edev_flag |= ENTL_DEVICE_FLAG_RETRY;
                    mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
                }
                else if (result == -1) {
                    entl_state_error(stm, ENTL_ERROR_FATAL);
                    dev->edev_flag |= ENTL_DEVICE_FLAG_SIGNAL;
                    mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
                }
                else {
                    dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_WAITING;
                }
            }
        }
    }

    return retval;
}

// Assumes NOT interrupt context
// process packet being sent. The ENTL message can only be sent over the single (non MSS) packet
static void entl_device_process_tx_packet(entl_device_t *dev, struct sk_buff *skb) {
    struct ethhdr *eth = (struct ethhdr *) skb->data;

    // MSS packet can't be used for ENTL message (will use a header over multiple packets)
    if (skb_is_gso(skb)) {
        uint16_t u_addr = ENTL_MESSAGE_NOP_U;
        uint32_t l_addr = 0;
        unsigned char d_addr[ETH_ALEN];
        d_addr[0] = u_addr >> 8;
        d_addr[1] = u_addr;
        d_addr[2] = l_addr >> 24;
        d_addr[3] = l_addr >> 16;
        d_addr[4] = l_addr >>  8;
        d_addr[5] = l_addr;
        memcpy(eth->h_dest, d_addr, ETH_ALEN);
    }
    else {
        struct entl_state_machine *stm = &dev->edev_stm;
        uint16_t u_addr;
        uint32_t l_addr;
        int ret = entl_next_send_tx(stm, &u_addr, &l_addr);
        if (ret & ENTL_ACTION_SIG_AIT) {
            dev->edev_flag |= ENTL_DEVICE_FLAG_SIGNAL2; // AIT send completion signal
        }
        unsigned char d_addr[ETH_ALEN];
        d_addr[0] = u_addr >> 8;
        d_addr[1] = u_addr;
        d_addr[2] = l_addr >> 24;
        d_addr[3] = l_addr >> 16;
        d_addr[4] = l_addr >>  8;
        d_addr[5] = l_addr;
        memcpy(eth->h_dest, d_addr, ETH_ALEN);
        if (u_addr != ENTL_MESSAGE_NOP_U) {
            dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_WAITING;
        }
    }
}

static int entl_do_ioctl(struct net_device *netdev, struct ifreq *ifr, int cmd) {
    struct e1000_adapter *adapter = netdev_priv(netdev);
    entl_device_t *dev = &adapter->entl_dev;
    struct e1000_hw *hw = &adapter->hw;
    struct entl_state_machine *stm = &dev->edev_stm;

    switch (cmd) {
    case SIOCDEVPRIVATE_ENTL_RD_CURRENT: {
        struct e1000_hw *hw = &adapter->hw;
        uint32_t link = !hw->mac.get_link_status; // FIXME: carrier?
        struct entl_ioctl_data entl_data;
        entl_data.link_state = link;
        entl_read_current_state(stm, &entl_data.state, &entl_data.error_state);
        entl_data.num_queued = entl_num_queued(stm);
        copy_to_user(ifr->ifr_data, &entl_data, sizeof(struct entl_ioctl_data));
    }
    break;

    case SIOCDEVPRIVATE_ENTL_RD_ERROR: {
        struct e1000_hw *hw = &adapter->hw;
        uint32_t link = !hw->mac.get_link_status;
        struct entl_ioctl_data entl_data;
        entl_data.link_state = link;
        entl_read_error_state(stm, &entl_data.state, &entl_data.error_state);
        entl_data.num_queued = entl_num_queued(stm);
        copy_to_user(ifr->ifr_data, &entl_data, sizeof(struct entl_ioctl_data));
        // dump_state("current", &entl_data.state, 1);
        // dump_state("error", &entl_data.error_state, 0);
    }
    break;

    case SIOCDEVPRIVATE_ENTL_SET_SIGRCVR: {
        struct entl_ioctl_data entl_data;
        copy_from_user(&entl_data, ifr->ifr_data, sizeof(struct entl_ioctl_data));
        dev->edev_user_pid = entl_data.pid;
    }
    break;

    case SIOCDEVPRIVATE_ENTL_GEN_SIGNAL:
    break;

    case SIOCDEVPRIVATE_ENTL_DO_INIT: {
        adapter->entl_flag = 1;
        entl_e1000_configure(adapter);
        uint32_t icr = er32(ICR);
        uint32_t ctrl = er32(CTRL);
        uint32_t ims = er32(IMS);
    }
    break;

    case SIOCDEVPRIVATE_ENTT_SEND_AIT: {
        struct entt_ioctl_ait_data *ait_data = kzalloc(sizeof(struct entt_ioctl_ait_data), GFP_ATOMIC);
        copy_from_user(ait_data, ifr->ifr_data, sizeof(struct entt_ioctl_ait_data));

        int ret = entl_send_AIT_message(stm, ait_data);
        ait_data->num_messages = ret;
        copy_to_user(ifr->ifr_data, ait_data, sizeof(struct entt_ioctl_ait_data));

        if (ret < 0) {
            kfree(ait_data); // FIXME: check for memory leak?
        }
    }
    break;

    case SIOCDEVPRIVATE_ENTT_READ_AIT: {
        struct entt_ioctl_ait_data *ait_data = entl_read_AIT_message(stm);
        if (ait_data) {
            copy_to_user(ifr->ifr_data, ait_data, sizeof(struct entt_ioctl_ait_data));
            kfree(ait_data);
        }
        else {
            struct entt_ioctl_ait_data dt;
            dt.num_messages = 0;
            dt.message_len = 0;
            dt.num_queued = entl_num_queued(stm);
            copy_to_user(ifr->ifr_data, &dt, sizeof(struct entt_ioctl_ait_data));
        }
    }
    break;

    default:
        ENTL_DEBUG("ENTL %s ioctl error: undefined cmd %d\n", netdev->name, cmd);
        break;
    }

    return 0;
}

// entl version of e1000_configure
static void entl_e1000_configure(struct e1000_adapter *adapter) {
        struct e1000_ring *rx_ring = adapter->rx_ring;
        entl_device_t *dev = &adapter->entl_dev;
        struct e1000_hw *hw = &adapter->hw;
        struct net_device *netdev = adapter->netdev;

        entl_e1000e_set_rx_mode(netdev);
#if defined(NETIF_F_HW_VLAN_TX) || defined(NETIF_F_HW_VLAN_CTAG_TX)
        e1000_restore_vlan(adapter);
#endif
        e1000_init_manageability_pt(adapter);

        // We don’t need immediate interrupt on Tx completion.
        // (unless buffer was full and quick responce is required, but that’s not likely)
        e1000_configure_tx(adapter);

#ifdef NETIF_F_RXHASH
        if (netdev->features & NETIF_F_RXHASH)
                e1000e_setup_rss_hash(adapter);
#endif
        entl_e1000_setup_rctl(adapter);
        entl_e1000_configure_rx(adapter);
        adapter->alloc_rx_buf(rx_ring, e1000_desc_unused(rx_ring), GFP_KERNEL);

        struct entl_state_machine *stm = &dev->edev_stm;
        entl_state_machine_init(stm);
// bj
        // strlcpy(stm->name, dev->edev_name, sizeof(stm->name)); // FIXME
        entl_e1000_set_my_addr(dev, netdev->dev_addr);

        // force to check the link status on kernel task
        hw->mac.get_link_status = true;
}

static void entl_e1000_set_my_addr(entl_device_t *dev, const uint8_t *addr) {
    struct entl_state_machine *stm = &dev->edev_stm;
    uint16_t u_addr = (uint16_t) addr[0] << 8
                               | addr[1];
    uint32_t l_addr = (uint32_t)addr[2] << 24
                    | (uint32_t)addr[3] << 16
                    | (uint32_t)addr[4] <<  8
                    | (uint32_t)addr[5];
    entl_set_my_adder(stm, u_addr, l_addr);
}

#ifdef ENTL_TX_ON_ENTL_ENABLE
static netdev_tx_t entl_tx_transmit(struct sk_buff *skb, struct net_device *netdev) {
    struct e1000_adapter *adapter = netdev_priv(netdev);
    entl_device_t *dev = &adapter->entl_dev;
    ENTL_skb_queue_t *q = &dev->edev_tx_skb_queue;

    if (ENTL_skb_queue_full(q)) {
        BUG_ON(q->count >= q->size);
        return NETDEV_TX_BUSY;
    }

    struct ethhdr *eth = (struct ethhdr *) skb->data;
    if ((eth->h_proto != ETH_P_ECLP) && (eth->h_proto != ETH_P_ECLD)) {
        dev_kfree_skb_any(skb);
        return NETDEV_TX_OK;
    }

    ENTL_skb_queue_back_push(q, skb);

    int avail = ENTL_skb_queue_unused(q);
    if (avail < 2) {
        netif_stop_queue(netdev);
        dev->edev_queue_stopped = 1;
        return NETDEV_TX_BUSY;
    }

    return NETDEV_TX_OK;
}
#endif

// internal

static int inject_message(entl_device_t *dev, uint16_t u_addr, uint32_t l_addr, int flag) {
    struct e1000_adapter *adapter = container_of(dev, struct e1000_adapter, entl_dev);
    if (test_bit(__E1000_DOWN, &adapter->state)) return 1;

    struct net_device *netdev = adapter->netdev;
    struct pci_dev *pdev = adapter->pdev;
    struct e1000_ring *tx_ring = adapter->tx_ring;
    if (e1000_desc_unused(tx_ring) < 3) return 1;

    struct entl_state_machine *stm = &dev->edev_stm;

    struct entt_ioctl_ait_data *ait_data;
    int len;
    if (flag & ENTL_ACTION_SEND_AIT) {
        ait_data = entl_next_AIT_message(stm);
        len = ETH_HLEN + ait_data->message_len + sizeof(uint32_t);
        if (len < ETH_ZLEN) len = ETH_ZLEN; // min 60 - include/uapi/linux/if_ether.h
    }
    else {
        ait_data = NULL;
        len = ETH_ZLEN;
    }

    len += ETH_FCS_LEN;
    struct sk_buff *skb = __netdev_alloc_skb(netdev, len, GFP_ATOMIC);
    if (!skb) {
        return -1;
    }

    skb->len = len;

    unsigned char d_addr[ETH_ALEN];
    d_addr[0] = (u_addr >> 8) | 0x80; // messege only
    d_addr[1] = u_addr;
    d_addr[2] = l_addr >> 24;
    d_addr[3] = l_addr >> 16;
    d_addr[4] = l_addr >>  8;
    d_addr[5] = l_addr;

    struct ethhdr *eth = (struct ethhdr *) skb->data;
    memcpy(eth->h_source, netdev->dev_addr, ETH_ALEN);
    memcpy(eth->h_dest, d_addr, ETH_ALEN);
    eth->h_proto = 0; // protocol type is not used anyway

    if (flag & ENTL_ACTION_SEND_AIT) {
        unsigned char *cp = skb->data + sizeof(struct ethhdr);
        unsigned char *payload = cp + sizeof(uint32_t);
        memcpy(cp, &ait_data->message_len, sizeof(uint32_t));
        memcpy(payload, ait_data->data, ait_data->message_len);
    }

    int i = adapter->tx_ring->next_to_use;
    struct e1000_buffer *buffer_info = &tx_ring->buffer_info[i];
    buffer_info->length = skb->len;
    buffer_info->time_stamp = jiffies;
    buffer_info->next_to_watch = i;
    buffer_info->dma = dma_map_single(&pdev->dev, skb->data, skb->len, DMA_TO_DEVICE);
    buffer_info->mapped_as_page = false;
    if (dma_mapping_error(&pdev->dev, buffer_info->dma)) {
        buffer_info->dma = 0;
        dev_kfree_skb_any(skb);
        return -1;
    }

    buffer_info->skb = skb;
    // report number of byte queued for sending to the device hardware queue
    netdev_sent_queue(netdev, skb->len);

    // process e1000_tx_queue
    uint32_t txd_upper = 0;
    uint32_t txd_lower = E1000_TXD_CMD_IFCS;
    struct e1000_tx_desc *tx_desc = E1000_TX_DESC(*tx_ring, i);
    tx_desc->buffer_addr = cpu_to_le64(buffer_info->dma);
    tx_desc->upper.data = cpu_to_le32(txd_upper);
    tx_desc->lower.data = cpu_to_le32(txd_lower | buffer_info->length);
    tx_desc->lower.data |= cpu_to_le32(adapter->txd_cmd);

    i++;
    if (i == tx_ring->count) i = 0;

    /* Force memory writes to complete before letting h/w know there are new descriptors to fetch.
     * (Only applicable for weak-ordered memory model archs, such as IA-64).
     */
    wmb();

    tx_ring->next_to_use = i;

    // Update TDT register in the NIC
    if (adapter->flags2 & FLAG2_PCIM2PCI_ARBITER_WA)
        e1000e_update_tdt_wa(tx_ring, tx_ring->next_to_use);
    else
        writel(tx_ring->next_to_use, tx_ring->tail);

    /* we need this if more than one processor can write to our tail at a time,
     * it synchronizes IO on IA64/Altix systems
     */
    mmiowb();
    return 0;
}

/*
 *  Author: Atsushi Kasuya
 *    Note: This code is written as .c but actually included in a part of netdevice.c in e1000e driver code
 *     so that it can share the static functions in the driver.
 */

static void entl_watchdog(unsigned long data) {
    entl_device_t *dev = (entl_device_t *)data;
    schedule_work(&dev->edev_watchdog_task); // use global kernel work queue
}

static void entl_watchdog_task(struct work_struct *work) {
    unsigned long wakeup = 1 * HZ;  // one second

    entl_device_t *dev = container_of(work, entl_device_t, edev_watchdog_task); // get the struct pointer from a member
    struct e1000_adapter *adapter = container_of(dev, struct e1000_adapter, entl_dev);

    if (!dev->edev_flag) {
        dev->edev_flag |= ENTL_DEVICE_FLAG_WAITING;
        goto restart_watchdog;
    }

    if ((dev->edev_flag & ENTL_DEVICE_FLAG_SIGNAL) && dev->edev_user_pid) {
            dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_SIGNAL;

            struct task_struct *t = pid_task(find_vpid(dev->edev_user_pid), PIDTYPE_PID);
            struct siginfo info;
            info.si_signo = SIGIO;
            info.si_int = 1;
            info.si_code = SI_QUEUE;
            if (t != NULL) send_sig_info(SIGUSR1, &info, t);
    }
    else if ((dev->edev_flag & ENTL_DEVICE_FLAG_SIGNAL2) && dev->edev_user_pid) {
            dev->edev_flag &= ~(uint32_t)ENTL_DEVICE_FLAG_SIGNAL2;

            struct task_struct *t = pid_task(find_vpid(dev->edev_user_pid), PIDTYPE_PID);
            struct siginfo info;
            info.si_signo = SIGIO;
            info.si_int = 1;
            info.si_code = SI_QUEUE;
            if (t != NULL) send_sig_info(SIGUSR2, &info, t);
    }

    if (netif_carrier_ok(adapter->netdev) && (dev->edev_flag & ENTL_DEVICE_FLAG_HELLO)) {
        struct e1000_ring *tx_ring = adapter->tx_ring;
        if (test_bit(__E1000_DOWN, &adapter->state)) {
            goto restart_watchdog;
        }

        int t;
        if ((t = e1000_desc_unused(tx_ring)) < 3) {
            goto restart_watchdog;
        }

        struct entl_state_machine *stm = &dev->edev_stm;
        uint32_t entl_state = FETCH_STATE(stm);
        if ((entl_state == ENTL_STATE_HELLO)
        ||  (entl_state == ENTL_STATE_WAIT)
        ||  (entl_state == ENTL_STATE_RECEIVE)
        ||  (entl_state == ENTL_STATE_AM)
        ||  (entl_state == ENTL_STATE_BH)) {
            uint16_t u_addr;
            uint32_t l_addr;
            int ret = entl_get_hello(stm, &u_addr, &l_addr);
            if (ret) {
                int result;
                unsigned long flags;
                spin_lock_irqsave(&adapter->entl_txring_lock, flags);
                result = inject_message(dev, u_addr, l_addr, ret);
                spin_unlock_irqrestore(&adapter->entl_txring_lock, flags);

                if (result == 0) {
                    dev->edev_flag &= ~(uint32_t)ENTL_DEVICE_FLAG_HELLO;
                }
            }
        }
        else {
            dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_HELLO;
        }
    }
    else if (dev->edev_flag & ENTL_DEVICE_FLAG_RETRY) {
        struct e1000_adapter *adapter = container_of(dev, struct e1000_adapter, entl_dev);
        if (test_bit(__E1000_DOWN, &adapter->state)) goto restart_watchdog;

        struct e1000_ring *tx_ring = adapter->tx_ring;
        if (e1000_desc_unused(tx_ring) < 3) goto restart_watchdog;

        int result;
        unsigned long flags;
        spin_lock_irqsave(&adapter->entl_txring_lock, flags);
        result = inject_message(dev, dev->edev_u_addr, dev->edev_l_addr, dev->edev_action);
        spin_unlock_irqrestore(&adapter->entl_txring_lock, flags);

        if (result == 0) {
            dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_RETRY;
            dev->edev_flag &= ~(uint32_t)ENTL_DEVICE_FLAG_WAITING;
        }
    }
    else if (dev->edev_flag & ENTL_DEVICE_FLAG_WAITING) {
        dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_WAITING;
        uint32_t entl_state = FETCH_STATE(stm);
        if ((entl_state == ENTL_STATE_HELLO)
        ||  (entl_state == ENTL_STATE_WAIT)
        ||  (entl_state == ENTL_STATE_RECEIVE)
        ||  (entl_state == ENTL_STATE_AM)
        ||  (entl_state == ENTL_STATE_BH)) {
            dev->edev_flag |= ENTL_DEVICE_FLAG_HELLO;
        }
    }

restart_watchdog:
    mod_timer(&dev->edev_watchdog_timer, round_jiffies(jiffies + wakeup));
}

// unused, debug
#if 0
static void dump_state(char *type, entl_state_t *st, int flag) {
    ENTL_DEBUG("%s event_i_know: %d  event_i_sent: %d event_send_next: %d current_state: %d error_flag %x p_error %x error_count %d @ %ld.%ld \n",
        type, st->event_i_know, st->event_i_sent, st->event_send_next, st->current_state, st->error_flag, st->p_error_flag, st->error_count, st->update_time.tv_sec, st->update_time.tv_nsec
    );

    if (st->error_flag) {
        ENTL_DEBUG("  Error time: %ld.%ld\n", st->error_time.tv_sec, st->error_time.tv_nsec);
    }
#ifdef ENTL_SPEED_CHECK
    if (flag) {
        ENTL_DEBUG("  interval_time    : %ld.%ld\n", st->interval_time.tv_sec, st->interval_time.tv_nsec);
        ENTL_DEBUG("  max_interval_time: %ld.%ld\n", st->max_interval_time.tv_sec, st->max_interval_time.tv_nsec);
        ENTL_DEBUG("  min_interval_time: %ld.%ld\n", st->min_interval_time.tv_sec, st->min_interval_time.tv_nsec);
    }
#endif
}
#endif

// deriviate work - ref: orig-frag-netdev.c, copied-frag-entl_device.c

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