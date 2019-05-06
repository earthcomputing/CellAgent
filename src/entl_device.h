#ifndef _ENTL_DEVICE_H_
#define _ENTL_DEVICE_H_

#define ENTL_DEVICE_FLAG_HELLO   0x0001
#define ENTL_DEVICE_FLAG_SIGNAL  0x0002
#define ENTL_DEVICE_FLAG_RETRY   0x0004
#define ENTL_DEVICE_FLAG_WAITING 0x0008
#define ENTL_DEVICE_FLAG_SIGNAL2 0x0010
#define ENTL_DEVICE_FLAG_FATAL   0x8000


typedef struct entl_device {
} entl_device_t;


#ifdef _IN_NETDEV_C_

// FIXME: netdev.c
#include "entl_user_api.h"

// forward declarations (static)
static void entl_device_init(entl_device_t *dev);
static void entl_device_link_down(entl_device_t *dev);
static void entl_device_link_up(entl_device_t *dev);
static bool entl_device_process_rx_packet(entl_device_t *dev, struct sk_buff *skb);
static void entl_device_process_tx_packet(entl_device_t *dev, struct sk_buff *skb);

static int entl_do_ioctl(struct net_device *netdev, struct ifreq *ifr, int cmd);

static void entl_e1000_configure(struct e1000_adapter *adapter);
static void entl_e1000_set_my_addr(entl_device_t *dev, const u8 *addr);

#ifdef ENTL_TX_ON_ENTL_ENABLE
static netdev_tx_t entl_tx_transmit(struct sk_buff *skb, struct net_device *netdev);
#endif

// dead code
// static int entl_tx_queue_has_data(entl_device_t *dev);
// static void entl_tx_pull(struct net_device *netdev);
#endif

#endif
