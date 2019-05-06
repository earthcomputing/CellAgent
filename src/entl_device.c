
#include "entl_skb_queue.h"

static bool entl_device_process_rx_packet(entl_device_t *dev, struct sk_buff *skb) {
    return true;
}

static void entl_device_process_tx_packet(entl_device_t *dev, struct sk_buff *skb) { }
static void entl_device_init(entl_device_t *dev) { }
static void entl_device_link_down(entl_device_t *dev) { }
static void entl_device_link_up(entl_device_t *dev) { }

static int entl_do_ioctl(struct net_device *netdev, struct ifreq *ifr, int cmd) {
    return 0;
}

static void entl_e1000_configure(struct e1000_adapter *adapter) { }
static void entl_e1000_set_my_addr(entl_device_t *dev, const u8 *addr) { }

#ifdef ENTL_TX_ON_ENTL_ENABLE
// returns NETDEV_TX_BUSY when
// returns NETDEV_TX_OK when
static netdev_tx_t entl_tx_transmit(struct sk_buff *skb, struct net_device *netdev) {
    return NETDEV_TX_OK;
}
#endif
