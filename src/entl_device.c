
#define FETCH_STATE(stm) 0 /* ((stm)->current_state.current_state) */

// newline should be unnecessary here - https://lwn.net/Articles/732420/
#define ENTL_DEBUG(fmt, args...) printk(KERN_ALERT "ENTL: " fmt "\n", ## args)
#define ADAPT_INFO(fmt, args...) printk(KERN_INFO "ADAPT: " fmt "\n", ## args)
#define ENTL_DEBUG_NAME(_name, fmt, args...) ENTL_DEBUG("%s " fmt, _name, ## args)
#define ADAPT_INFO_NAME(_name, fmt, args...) ADAPT_INFO("%s " fmt, _name, ## args)
#define ADAPTER_DEBUG(adapter, fmt, args...) ENTL_DEBUG_NAME(adapter->netdev->name, fmt, ## args)

#include "entl_skb_queue.h"
#include "entl_state_machine.h"
#include "entl_ioctl.h"
#include "entl_device.h"
#include "entl_user_api.h"

#include "netdev_entl_if.h"
#include "entl_stm_if.h"

#define ENTL_ADAPT_IMPL
#include "ecnl_entl_if.h"

// copied e1000 routines:
static void entl_e1000_configure(struct e1000_adapter *adapter);
static void entl_e1000e_set_rx_mode(struct net_device *netdev);
static void entl_e1000_setup_rctl(struct e1000_adapter *adapter);
static void entl_e1000_configure_rx(struct e1000_adapter *adapter);

// forward declarations (internal/private)
static int inject_message(entl_device_t *dev, uint16_t emsg_raw, uint32_t seqno, int send_action);
#ifndef BIONIC_3421
static void entl_watchdog(unsigned long data);
#else
static void entl_watchdog(struct timer_list *t);
#endif
static void entl_watchdog_task(struct work_struct *work);
// static void dump_state(char *type, entl_state_t *st, int flag); // debug

// in theory, only the STM can do 'inject_message',
// however we also have to guard against other driver actions (interrupts, hw maint)
// STM_LOCK - stm->state_lock
static inline int locked_inject(entl_device_t *dev, struct e1000_adapter *adapter, uint16_t emsg_raw, uint32_t seqno, int send_action) {
    unsigned long flags;
    spin_lock_irqsave(&adapter->entl_txring_lock, flags);
    int inject_action = inject_message(dev, emsg_raw, seqno, send_action);
    spin_unlock_irqrestore(&adapter->entl_txring_lock, flags);
    return inject_action;
}

static char *emsg_names[] = {
    "HELLO", // 0x0000
    "EVENT", // 0x0001
    "NOP",   // 0x0002
    "AIT",   // 0x0003
    "ACK"    // 0x0004
};

static inline char *emsg_op(uint16_t u_daddr) {
    int opnum = get_entl_msg(u_daddr);
    return (opnum < 5) ? emsg_names[opnum] : "??";
}

static char *letters = "0123456789abcdef";
extern void dump_ait_data(entl_state_machine_t *stm, char *tag, struct entt_ioctl_ait_data *ait_data) {
    void *d = ait_data->data;
    int nbytes = ait_data->message_len;
    int msgs = ait_data->num_messages;
    int queued = ait_data->num_queued;
    char window[3*41];
    int f = 0;
    for (int i = 0; i < nbytes; i++) {
        char ch = ((char *) d)[i] & 0xff;
        int n0 = (ch & 0xf0) >> 4;
        int n1 = (ch & 0x0f);
        window[f+0] = ' ';
        window[f+1] = letters[n0];
        window[f+2] = letters[n1];
        window[f+3] = '\0';
        f += 3;
        if (f >= 3*40) break;
    }
    ENTL_DEBUG_NAME(stm->name, "%s - msgs: %d queued: %d nbytes: %d - %s", tag, msgs, queued, nbytes, window);
}

// inline helpers:
static inline void unpack_eth(const uint8_t *p, uint16_t *u, uint32_t *l) {
    uint16_t mac_hi = (uint16_t) p[0] << 8
                    | (uint16_t) p[1];
    uint32_t mac_lo = (uint32_t) p[2] << 24
                    | (uint32_t) p[3] << 16
                    | (uint32_t) p[4] <<  8
                    | (uint32_t) p[5];
    *u = mac_hi;
    *l = mac_lo;
}

static inline void encode_dest(uint8_t *h_dest, uint16_t mac_hi, uint32_t mac_lo) {
    unsigned char mac_addr[ETH_ALEN];
    mac_addr[0] = (mac_hi >>  8) & 0xff;
    mac_addr[1] = (mac_hi)       & 0xff;
    mac_addr[2] = (mac_lo >> 24) & 0xff;
    mac_addr[3] = (mac_lo >> 16) & 0xff;
    mac_addr[4] = (mac_lo >>  8) & 0xff;
    mac_addr[5] = (mac_lo)       & 0xff;
    memcpy(h_dest, mac_addr, ETH_ALEN);
}

// netdev entry points: (from e1000_probe) w/adapter->entl_dev
static void entl_device_init(entl_device_t *edev) {
    memset(edev, 0, sizeof(struct entl_device));

    entl_state_machine_t *stm = &edev->edev_stm;
    entl_state_machine_init(stm); // FIXME: huh?

    // FIXME: name not set until register_netdev ??
    // size_t elen = strlcpy(edev->edev_name, edev->name, sizeof(edev->edev_name));
    // size_t slen = strlcpy(stm->name, edev->edev_name, sizeof(stm->name));

    // watchdog timer & task setup
#ifndef BIONIC_3421
    init_timer(&edev->edev_watchdog_timer);
    edev->edev_watchdog_timer.function = entl_watchdog;
    edev->edev_watchdog_timer.data = (unsigned long) edev;
#else
    timer_setup(&edev->edev_watchdog_timer, entl_watchdog, 0);
#endif
    INIT_WORK(&edev->edev_watchdog_task, entl_watchdog_task);
    ENTL_skb_queue_init(&edev->edev_tx_skb_queue);
    edev->edev_queue_stopped = 0;
}

static void entl_device_link_down(entl_device_t *dev) {
    entl_state_machine_t *stm = &dev->edev_stm;
    entl_state_error(stm, ENTL_ERROR_FLAG_LINKDONW);
    dev->edev_flag = ENTL_DEVICE_FLAG_SIGNAL;
    mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
}

static void entl_device_link_up(entl_device_t *dev) {
    entl_state_machine_t *stm = &dev->edev_stm;
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

    unsigned int len = skb->len;
    if (len <= sizeof(struct ethhdr)) {
        // FIXME
        ENTL_DEBUG_NAME(dev->edev_stm.name, "process_rx - runt len %d", len);
    }

    uint16_t smac_hi; uint32_t smac_lo; unpack_eth(eth->h_source, &smac_hi, &smac_lo);
    uint16_t emsg_raw; uint32_t seqno; unpack_eth(eth->h_dest, &emsg_raw, &seqno);

    bool retval = true;
    if (emsg_raw & ENTL_MESSAGE_ONLY_U) retval = false;

    entl_state_machine_t *stm = &dev->edev_stm;
    int recv_action = entl_received(stm, smac_hi, smac_lo, emsg_raw, seqno);

    if (recv_action == ENTL_ACTION_ERROR) {
        dev->edev_flag |= (ENTL_DEVICE_FLAG_HELLO | ENTL_DEVICE_FLAG_SIGNAL);
        mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
        return retval;
    }

    if (recv_action == ENTL_ACTION_SIG_ERR) {
        dev->edev_flag |= ENTL_DEVICE_FLAG_SIGNAL;
        mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
        return retval;
    }

    if (recv_action & ENTL_ACTION_PROC_AIT) {
        unsigned int len = skb->len;
        if (len <= sizeof(struct ethhdr)) {
            // FIXME
        }
        else {
            struct entt_ioctl_ait_data *ait_data = kzalloc(sizeof(struct entt_ioctl_ait_data), GFP_ATOMIC);
            unsigned char *level0 = skb->data + sizeof(struct ethhdr);
            uint32_t nbytes;
            memcpy(&nbytes, level0, sizeof(uint32_t));
// FIXME: MAX_AIT_MESSAGE_SIZE 256
            if ((nbytes > 0) && (nbytes < MAX_AIT_MESSAGE_SIZE)) {
                unsigned char *payload = level0 + sizeof(uint32_t);
                ait_data->message_len = nbytes;
                memcpy(ait_data->data, payload, nbytes);
                // ait_data->num_messages = 0;
                // ait_data->num_queued = 0;
dump_ait_data(stm, "process_rx - recv", ait_data);
            }
            else {
                ait_data->message_len = 0;
                // ait_data->num_messages = 0;
                // ait_data->num_queued = 0;
            }
ENTL_DEBUG_NAME(stm->name, "process_rx - message 0x%04x (%s) seqno %d payload len %d", emsg_raw, emsg_op(emsg_raw), seqno, ait_data->message_len);
            entl_new_AIT_message(stm, ait_data);
        }
    }

    if (recv_action & ENTL_ACTION_SIG_AIT) {
        dev->edev_flag |= ENTL_DEVICE_FLAG_SIGNAL2;
    }

    if (recv_action & ENTL_ACTION_SEND) {
        // SEND_DAT is set on SEND state to check if TX queue has data
        if (recv_action & ENTL_ACTION_SEND_DAT && ENTL_skb_queue_has_data(&dev->edev_tx_skb_queue)) {
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
                // tx queue empty, inject a new packet
                uint16_t emsg_raw = (uint16_t) -1; uint32_t seqno = (uint32_t) -1;
                int send_action = entl_next_send(stm, &emsg_raw, &seqno);

                if (get_entl_msg(emsg_raw) != ENTL_MESSAGE_NOP_U) {
                    int inject_action = locked_inject(dev, adapter, emsg_raw, seqno, send_action);
                    // failed inject, invoke task
                    if (inject_action == 1) {
                        // resource error, retry
                        dev->edev_u_addr = emsg_raw;
                        dev->edev_l_addr = seqno;
                        dev->edev_action = send_action;
                        dev->edev_flag |= ENTL_DEVICE_FLAG_RETRY;
                        mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
                    }
                    else if (inject_action == -1) {
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
            uint16_t emsg_raw = (uint16_t) -1; uint32_t seqno = (uint32_t) -1;
            int send_action = entl_next_send(stm, &emsg_raw, &seqno);

            if (get_entl_msg(emsg_raw) != ENTL_MESSAGE_NOP_U) {
                int inject_action = locked_inject(dev, adapter, emsg_raw, seqno, send_action);
                // failed inject, invoke task
                if (inject_action == 1) {
                    // resource error, so retry
                    dev->edev_u_addr = emsg_raw;
                    dev->edev_l_addr = seqno;
                    dev->edev_action = send_action;
                    dev->edev_flag |= ENTL_DEVICE_FLAG_RETRY;
                    mod_timer(&dev->edev_watchdog_timer, jiffies + 1);
                }
                else if (inject_action == -1) {
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

// from patch/netdev.c:
// Assumes NOT interrupt context
// process packet being sent. The ENTL message can only be sent over the single (non MSS) packet
static void entl_device_process_tx_packet(entl_device_t *dev, struct sk_buff *skb) {
    struct ethhdr *eth = (struct ethhdr *) skb->data;

    // maximum segment size (MSS) packet can't be used for ENTL message (will use a header over multiple packets)
    if (skb_is_gso(skb)) {
        encode_dest(eth->h_dest, ENTL_MESSAGE_NOP_U, 0);
    }
    else {
        entl_state_machine_t *stm = &dev->edev_stm;

// might be offline(no carrier), or be newly online after offline ??
        uint16_t emsg_raw = (uint16_t) -1; uint32_t seqno = (uint32_t) -1;
        int send_action = entl_next_send_tx(stm, &emsg_raw, &seqno);
        encode_dest(eth->h_dest, emsg_raw, seqno);

        if (send_action & ENTL_ACTION_SIG_AIT) {
            dev->edev_flag |= ENTL_DEVICE_FLAG_SIGNAL2; // AIT send completion signal
        }

        if (emsg_raw != ENTL_MESSAGE_NOP_U) {
ENTL_DEBUG_NAME(stm->name, "process_tx - message 0x%04x (%s) seqno %d", emsg_raw, emsg_op(emsg_raw), seqno);
            dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_WAITING;
        }
    }
}

static int entl_do_ioctl(struct net_device *netdev, struct ifreq *ifr, int cmd) {
    struct e1000_adapter *adapter = netdev_priv(netdev);
    entl_device_t *dev = &adapter->entl_dev;
    struct e1000_hw *hw = &adapter->hw;
    entl_state_machine_t *stm = &dev->edev_stm;

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

ENTL_DEBUG_NAME(stm->name, "ioctl - entl_send_AIT_message");
        int q_space = entl_send_AIT_message(stm, ait_data);
        // result data
        ait_data->num_messages = q_space;
        ait_data->num_queued = -1;
        copy_to_user(ifr->ifr_data, ait_data, sizeof(struct entt_ioctl_ait_data));
dump_ait_data(stm, "ioctl - sendq_push", ait_data);

        if (q_space < 0) {
            kfree(ait_data); // FIXME: check for memory leak?
        }
    }
    break;

    case SIOCDEVPRIVATE_ENTT_READ_AIT: {
ENTL_DEBUG_NAME(stm->name, "ioctl - entl_read_AIT_message");
        struct entt_ioctl_ait_data *ait_data = entl_read_AIT_message(stm); // recvq_pop
        if (ait_data) {
dump_ait_data(stm, "ioctl - recvq_pop", ait_data);
            copy_to_user(ifr->ifr_data, ait_data, sizeof(struct entt_ioctl_ait_data));
            kfree(ait_data);
        }
        else {
            struct entt_ioctl_ait_data dt;
            dt.message_len = 0;
            dt.num_messages = 0;
            dt.num_queued = entl_num_queued(stm);
            copy_to_user(ifr->ifr_data, &dt, sizeof(struct entt_ioctl_ait_data));
        }
    }
    break;

    default:
        ENTL_DEBUG_NAME(netdev->name, "ioctl error: undefined cmd %d", cmd);
        break;
    }

    return 0;
}

// name here is problematical - called before register_netdev (but after netdev->name = pci_name)
// called both from netdev (patch) and edev_init
static void entl_e1000_set_my_addr(struct e1000_adapter *adapter, const uint8_t *addr) {
    ENTL_DEBUG_NAME(adapter->netdev->name, "init - macaddr %02x:%02x:%02x:%02x:%02x:%02x", addr[0], addr[1], addr[2], addr[3], addr[4], addr[5]);
    uint16_t u_addr; uint32_t l_addr; unpack_eth(addr, &u_addr, &l_addr);

    // FIXME: mcn name not set up ??
    entl_device_t *edev = &adapter->entl_dev;
    entl_state_machine_t *stm = &edev->edev_stm;
    entl_set_my_adder(stm, u_addr, l_addr);
}

// netdev entry points: (from entl_e1000_configure)
static void edev_init(struct e1000_adapter *adapter) {
    struct net_device *netdev = adapter->netdev;
    entl_device_t *edev = &adapter->entl_dev;
    entl_state_machine_t *stm = &edev->edev_stm;

    ENTL_DEBUG_NAME(netdev->name, "edev_init");

    // update name(s) to match adapter->netdev->name
    size_t elen = strlcpy(edev->edev_name, netdev->name, sizeof(edev->edev_name));
    size_t slen = strlcpy(stm->name, edev->edev_name, sizeof(stm->name));

    entl_e1000_set_my_addr(adapter, netdev->dev_addr);
#if 0
    // force to check the link status on kernel task (taken care of elsewhere)
    struct e1000_hw *hw = &adapter->hw;
    hw->mac.get_link_status = true;
#endif
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

// FIXME - I hate to do debug printf's here but:
// https://github.com/torvalds/linux/blob/master/kernel/printk/printk.c#L2023
// process_tx - message 0x0000 seqno 0

// emsg_raw, seqno
static int inject_message(entl_device_t *dev, uint16_t emsg_raw, uint32_t seqno, int send_action) {
    struct e1000_adapter *adapter = container_of(dev, struct e1000_adapter, entl_dev);
    if (test_bit(__E1000_DOWN, &adapter->state)) return 1;

    struct net_device *netdev = adapter->netdev;
    struct pci_dev *pdev = adapter->pdev;
    struct e1000_ring *tx_ring = adapter->tx_ring;
    if (e1000_desc_unused(tx_ring) < 3) return 1;

    entl_state_machine_t *stm = &dev->edev_stm;

    struct entt_ioctl_ait_data *ait_data;
    int len;
    if (send_action & ENTL_ACTION_SEND_AIT) {
ENTL_DEBUG_NAME(stm->name, "inject - entl_next_AIT_message (%s)", emsg_op(emsg_raw));
        ait_data = entl_next_AIT_message(stm); // fetch payload data
        len = ETH_HLEN + ait_data->message_len + sizeof(uint32_t);
// FIXME: here we know how bit the actual frame will be ; last chance to discard it
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

    struct ethhdr *eth = (struct ethhdr *) skb->data;
    memcpy(eth->h_source, netdev->dev_addr, ETH_ALEN);
    emsg_raw |= 0x8000; // message only
    encode_dest(eth->h_dest, emsg_raw, seqno);
    eth->h_proto = 0; // ETH_P_ECLP : protocol type is not used anyway

    if (ait_data) {
        // copy ait_data payload into skb
        if (send_action & ENTL_ACTION_SEND_AIT) {
            unsigned char *level0 = skb->data + sizeof(struct ethhdr);
            unsigned char *payload = level0 + sizeof(uint32_t);
            memcpy(level0, &ait_data->message_len, sizeof(uint32_t));
            memcpy(payload, ait_data->data, ait_data->message_len);
dump_ait_data(stm, "tx_ring - inject", ait_data);
        }
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

#ifndef BIONIC_3421
static void entl_watchdog(unsigned long data) {
    entl_device_t *dev = (entl_device_t *)data;
    schedule_work(&dev->edev_watchdog_task); // use global kernel work queue
}
#else
static void entl_watchdog(struct timer_list *t) {
    entl_device_t *dev = from_timer(dev, t, edev_watchdog_timer);
    schedule_work(&dev->edev_watchdog_task); // use global kernel work queue
}
#endif

static inline void notify_listener(int subscriber, int sigusr) {
    struct siginfo info = {
        .si_signo = SIGIO,
        .si_int = 1,
        .si_code = SI_QUEUE
    };
    struct task_struct *task = pid_task(find_vpid(subscriber), PIDTYPE_PID);
    if (task != NULL) send_sig_info(sigusr, &info, task);
}

static void entl_watchdog_task(struct work_struct *work) {
    unsigned long wakeup = 1 * HZ;  // one second

    entl_device_t *dev = container_of(work, entl_device_t, edev_watchdog_task); // get the struct pointer from a member
    struct e1000_adapter *adapter = container_of(dev, struct e1000_adapter, entl_dev);

    if (!dev->edev_flag) {
        dev->edev_flag |= ENTL_DEVICE_FLAG_WAITING;
        goto restart_watchdog;
    }

    int subscriber = dev->edev_user_pid;
    if (subscriber) {
        if (dev->edev_flag & ENTL_DEVICE_FLAG_SIGNAL) {
            dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_SIGNAL;
            notify_listener(subscriber, SIGUSR1);
        }
        else if (dev->edev_flag & ENTL_DEVICE_FLAG_SIGNAL2) {
            dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_SIGNAL2;
            notify_listener(subscriber, SIGUSR2);
        }
    }

    // notice carrier (i.e. link up)
    if (netif_carrier_ok(adapter->netdev) && (dev->edev_flag & ENTL_DEVICE_FLAG_HELLO)) {
        struct e1000_ring *tx_ring = adapter->tx_ring;
        if (test_bit(__E1000_DOWN, &adapter->state)) {
            goto restart_watchdog;
        }

        int t;
        if ((t = e1000_desc_unused(tx_ring)) < 3) {
            goto restart_watchdog;
        }

        entl_state_machine_t *stm = &dev->edev_stm;
        uint32_t entl_state = FETCH_STATE(stm);

        if ((entl_state == ENTL_STATE_HELLO)
        ||  (entl_state == ENTL_STATE_WAIT)
        ||  (entl_state == ENTL_STATE_RECEIVE)
        ||  (entl_state == ENTL_STATE_AM)
        ||  (entl_state == ENTL_STATE_BH)) {
            uint16_t emsg_raw = (uint16_t) -1; uint32_t seqno = (uint32_t) -1;
            int send_action = entl_get_hello(stm, &emsg_raw, &seqno);
            if (send_action) {
                int inject_action = locked_inject(dev, adapter, emsg_raw, seqno, send_action);
                if (inject_action == 0) {
                    dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_HELLO;
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

        // uint16_t emsg_raw = dev->edev_u_addr;
        // uint32_t seqno = dev->edev_l_addr;
        // int action = dev->edev_action;
        int inject_action = locked_inject(dev, adapter, dev->edev_u_addr, dev->edev_l_addr, dev->edev_action);

        if (inject_action == 0) {
            dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_RETRY;
            dev->edev_flag &= ~(uint32_t) ENTL_DEVICE_FLAG_WAITING;
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
    ENTL_DEBUG(
        "%s"
        " event_i_know: %d"
        " event_i_sent: %d"
        " event_send_next: %d"
        " current_state: %d"
        " error_flag %x"
        " p_error %x"
        " error_count %d"
        " @ %ld.%ld",

        type,
        st->event_i_know,
        st->event_i_sent,
        st->event_send_next,
        st->current_state,
        st->error_flag,
        st->p_error_flag,
        st->error_count,
        st->update_time.tv_sec, st->update_time.tv_nsec
    );

    if (st->error_flag) {
        ENTL_DEBUG("  Error time: %ld.%ld", st->error_time.tv_sec, st->error_time.tv_nsec);
    }
#ifdef ENTL_SPEED_CHECK
    if (flag) {
        ENTL_DEBUG("  interval_time    : %ld.%ld", st->interval_time.tv_sec, st->interval_time.tv_nsec);
        ENTL_DEBUG("  max_interval_time: %ld.%ld", st->max_interval_time.tv_sec, st->max_interval_time.tv_nsec);
        ENTL_DEBUG("  min_interval_time: %ld.%ld", st->min_interval_time.tv_sec, st->min_interval_time.tv_nsec);
    }
#endif
}
#endif

// derivative work - ref: orig-frag-netdev.c, copied-frag-entl_device.c

// entl version of e1000_configure
/**
 * e1000_configure - configure the hardware for Rx and Tx
 * @adapter: private board structure
 **/
static void entl_e1000_configure(struct e1000_adapter *adapter) {
        struct e1000_ring *rx_ring = adapter->rx_ring;

        entl_e1000e_set_rx_mode(adapter->netdev);
#if defined(NETIF_F_HW_VLAN_TX) || defined(NETIF_F_HW_VLAN_CTAG_TX)
        e1000_restore_vlan(adapter);
#endif
        e1000_init_manageability_pt(adapter);

        // We don’t need immediate interrupt on Tx completion.
        // (unless buffer was full and quick responce is required, but that’s not likely)
        e1000_configure_tx(adapter);

#ifdef NETIF_F_RXHASH
        if (adapter->netdev->features & NETIF_F_RXHASH)
                e1000e_setup_rss_hash(adapter);
#endif
        entl_e1000_setup_rctl(adapter);
        entl_e1000_configure_rx(adapter);
        adapter->alloc_rx_buf(rx_ring, e1000_desc_unused(rx_ring), GFP_KERNEL);
#ifdef ENTL
        edev_init(adapter);
#endif
}

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

        ADAPTER_DEBUG(adapter, "entl_e1000e_set_rx_mode RCTL = %08x", rctl);
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
		ADAPTER_DEBUG(adapter, "entl_e1000_setup_rctl %d <= %d", adapter->netdev->mtu, ETH_DATA_LEN);
		rctl &= ~E1000_RCTL_LPE;
	}
	else {
		ADAPTER_DEBUG(adapter, "entl_e1000_setup_rctl %d > %d", adapter->netdev->mtu, ETH_DATA_LEN);
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

		ADAPTER_DEBUG(adapter, "entl_e1000_setup_rctl Workaround Si errata on 82577/82578 - configure IPG for jumbos");

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
		ADAPTER_DEBUG(adapter, "entl_e1000_setup_rctl E1000_RCTL_SZ_2048");
		rctl |= E1000_RCTL_SZ_2048;
		rctl &= ~E1000_RCTL_BSEX;
		break;
	case 4096:
		ADAPTER_DEBUG(adapter, "entl_e1000_setup_rctl E1000_RCTL_SZ_4096");
		rctl |= E1000_RCTL_SZ_4096;
		break;
	case 8192:
		ADAPTER_DEBUG(adapter, "entl_e1000_setup_rctl E1000_RCTL_SZ_8192");
		rctl |= E1000_RCTL_SZ_8192;
		break;
	case 16384:
		ADAPTER_DEBUG(adapter, "entl_e1000_setup_rctl E1000_RCTL_SZ_16384");
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

	ADAPTER_DEBUG(adapter, "entl_e1000_setup_rctl rx_ps_pages = %d", adapter->rx_ps_pages);

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
        ADAPTER_DEBUG(adapter, "entl_e1000_setup_rctl RCTL = %08x", rctl);

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
		ADAPTER_DEBUG(adapter, "entl_e1000_configure_rx use e1000_alloc_rx_buffers_ps");
#ifdef CONFIG_E1000E_NAPI
	} else if (adapter->netdev->mtu > ETH_FRAME_LEN + ETH_FCS_LEN) {
		rdlen = rx_ring->count * sizeof(union e1000_rx_desc_extended);
		adapter->clean_rx = e1000_clean_jumbo_rx_irq;
		adapter->alloc_rx_buf = e1000_alloc_jumbo_rx_buffers;
		ADAPTER_DEBUG(adapter, "entl_e1000_configure_rx use e1000_alloc_jumbo_rx_buffers");
#endif
	} else {
		rdlen = rx_ring->count * sizeof(union e1000_rx_desc_extended);
		adapter->clean_rx = e1000_clean_rx_irq;
		adapter->alloc_rx_buf = e1000_alloc_rx_buffers;
		ADAPTER_DEBUG(adapter, "entl_e1000_configure_rx use e1000_alloc_rx_buffers");
	}

	/* disable receives while setting up the descriptors */
	rctl = er32(RCTL);
	if (!(adapter->flags2 & FLAG2_NO_DISABLE_RX))
		ew32(RCTL, rctl & ~E1000_RCTL_EN);
	e1e_flush();
	usleep_range(10000, 20000);

	if (adapter->flags2 & FLAG2_DMA_BURST) {
		ADAPTER_DEBUG(adapter, "entl_e1000_configure_rx set DMA burst");
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
	ADAPTER_DEBUG(adapter, "entl_e1000_configure_rx set Receive Delay Timer Register = %d", adapter->rx_int_delay);
	ew32(RDTR, adapter->rx_int_delay);

	/* irq moderation */
	ADAPTER_DEBUG(adapter, "entl_e1000_configure_rx set Abs Delay Timer Register = %d", adapter->rx_abs_int_delay);
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

		ADAPTER_DEBUG(adapter, "entl_e1000_configure_rx adapter->netdev->mtu %d > ETH_DATA_LEN %d lat = %d", adapter->netdev->mtu, ETH_DATA_LEN, lat);

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
		ADAPTER_DEBUG(adapter, "entl_e1000_configure_rx adapter->netdev->mtu %d <= ETH_DATA_LEN %d default qos = %d", adapter->netdev->mtu, ETH_DATA_LEN, PM_QOS_DEFAULT_VALUE);

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
	ADAPTER_DEBUG(adapter, "entl_e1000_configure_rx RCTL = %08x", rctl);

	/* Enable Receives */
	ew32(RCTL, rctl);
}

// ENTL - ECNL linkage

// minimal i/f compatibility
static int adapt_validate(struct net_device *e1000e, int magic) {
    ADAPT_INFO_NAME(e1000e->name, "adapt_validate");

    struct e1000_adapter *adapter = netdev_priv(e1000e);
    if (adapter == NULL) return -1;
    entl_device_t *entl_dev = &adapter->entl_dev;
    if (entl_dev == NULL) return -1;
    entl_state_machine_t *stm = &entl_dev->edev_stm;
    if (stm == NULL) return -1;

    return (magic == ENCL_ENTL_MAGIC) ? 1 : -1; // ENCL_ENTL_MAGIC 0x5affdead
}

// ref: linux/netdevice.h - enum netdev_tx
static netdev_tx_t adapt_start_xmit(struct sk_buff *skb, struct net_device *e1000e) {
    ADAPT_INFO_NAME(e1000e->name, "adapt_start_xmit");
    if (skb == NULL) return -1;

#if 0
    return NETDEV_TX_OK; // 0x00
#endif
    return NETDEV_TX_BUSY; // 0x10
}

// edf_send_AIT
static int adapt_send_AIT(struct sk_buff *skb, struct net_device *e1000e) {
    ADAPT_INFO_NAME(e1000e->name, "adapt_send_AIT");
    if (skb == NULL) return -1;

    struct e1000_adapter *adapter = netdev_priv(e1000e);
    if (adapter == NULL) return -1;
    entl_device_t *entl_dev = &adapter->entl_dev;
    if (entl_dev == NULL) return -1;
    entl_state_machine_t *stm = &entl_dev->edev_stm;
    if (stm == NULL) return -1;

    // FIXME : nl_ecnl_send_ait_message and ecnl_forward_ait_message disagree about the type of 'skb' here.
    struct ec_ait_data *dt = (struct ec_ait_data *) skb;

    int nbytes = dt->ecad_message_len;
    if (nbytes > MAX_AIT_MESSAGE_SIZE) {
        nbytes = MAX_AIT_MESSAGE_SIZE;
        ADAPT_INFO_NAME(e1000e->name, "adapt_send_AIT oversize frame: %d truncated (%d)", dt->ecad_message_len, nbytes);
    }

    struct entt_ioctl_ait_data *ait_data = kzalloc(sizeof(struct entt_ioctl_ait_data), GFP_ATOMIC);
    memcpy(ait_data->data, dt->ecad_data, nbytes);
    ait_data->message_len = nbytes; // inject_message : memcpy(payload, ait_data->data, ait_data->message_len);

    int q_space = entl_send_AIT_message(stm, ait_data); // sendq_push
    ait_data->num_messages = q_space;
    ait_data->num_queued = -1;

// ADAPT_INFO_NAME(e1000e->name, "send_AIT skb: %px", skb);
dump_ait_data(stm, "adapt_send - sendq_push", ait_data);

    // FIXME : return q_space to caller ??
    if (q_space < 0) {
        // kfree(ait_data); // FIXME: check for memory leak?
        return -1;
    }

    return 0;
}

// edf_retrieve_AIT
static int adapt_retrieve_AIT(struct net_device *e1000e, ec_ait_data_t *data) {
    ADAPT_INFO_NAME(e1000e->name, "adapt_retrieve_AIT");
    if (data == NULL) return -1;

    struct e1000_adapter *adapter = netdev_priv(e1000e);
    if (adapter == NULL) return -1;
    entl_device_t *entl_dev = &adapter->entl_dev;
    if (entl_dev == NULL) return -1;
    entl_state_machine_t *stm = &entl_dev->edev_stm;
    if (stm == NULL) return -1;

    struct entt_ioctl_ait_data *ait_data = entl_read_AIT_message(stm); // recvq_pop
    if (ait_data) {

// ADAPT_INFO_NAME(e1000e->name, "retr_AIT skb: %px", data);
dump_ait_data(stm, "adapt_retr - recvq_pop", ait_data);

        memcpy(data, ait_data, sizeof(struct entt_ioctl_ait_data));
        kfree(ait_data);
    }
    else {
        struct entt_ioctl_ait_data dt;
        dt.message_len = 0;
        dt.num_messages = 0;
        dt.num_queued = entl_num_queued(stm);
        memcpy(data, &dt, sizeof(struct entt_ioctl_ait_data));
    }

    return 0;
}

static int adapt_write_reg(struct net_device *e1000e, ec_alo_reg_t *reg) {
    ADAPT_INFO_NAME(e1000e->name, "adapt_write_reg");
    if (reg == NULL) return -1;

#if 0
#endif
    return 0;
}

static int adapt_read_regset(struct net_device *e1000e, ec_alo_regs_t *regs) {
    ADAPT_INFO_NAME(e1000e->name, "adapt_read_regset");
    if (regs == NULL) return -1;

#if 0
#endif
    return 0;
}

static int adapt_get_state(struct net_device *e1000e, ec_state_t *state) {
    ADAPT_INFO_NAME(e1000e->name, "adapt_get_state");

    if (state == NULL) return -1;

    // ref: e1000e-3.3.4/src/e1000.h
    struct e1000_adapter *adapter = netdev_priv(e1000e);
    // ADAPT_INFO("  adapter: %p", adapter);
    if (adapter == NULL) return -1;

    entl_device_t *entl_dev = &adapter->entl_dev;
    // ADAPT_INFO("  entl_dev: %p", entl_dev);
    if (entl_dev == NULL) return -1;

    ADAPT_INFO("  entl_dev->edev_name: \"%s\"", entl_dev->edev_name);
    ADAPT_INFO("  entl_dev->edev_queue_stopped: %d", entl_dev->edev_queue_stopped);

    char *nic_name = adapter->netdev->name;
    int link_state = netif_carrier_ok(e1000e);
    state->ecs_link_state = link_state; // FIXME: raw data for now
    if (link_state) {
        int link_speed = adapter->link_speed;
        int link_duplex = adapter->link_duplex;
        ADAPT_INFO_NAME(nic_name, "NIC Link is Up %d Mbps %s Duplex", link_speed, (link_duplex == FULL_DUPLEX) ? "Full" : "Half");
    }
    else {
        ADAPT_INFO_NAME(nic_name, "NIC Link is Down");
    }

    entl_state_machine_t *stm = &entl_dev->edev_stm;
    if (stm == NULL) return -1;

#if 0
    // ref: add_link_state
    state->ecs_link_state = link_state;
    state->ecs_s_count = s_count;
    state->ecs_r_count = r_count;
    state->ecs_recover_count = recover_count;
    state->ecs_recovered_count = recovered_count;
    state->ecs_entt_count = entt_count;
    state->ecs_aop_count = aop_count;
    state->ecs_num_queued = num_queued;
    // FIXME: update_time, interval_time, max_interval_time, min_interval_time
#endif
    return 0;
}
