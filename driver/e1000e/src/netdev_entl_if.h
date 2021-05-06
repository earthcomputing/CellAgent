/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
#ifndef _NETDEV_ENTL_IF_H_
#define _NETDEV_ENTL_IF_H_

#ifdef _IN_NETDEV_C_

// back references to netdev.c
static netdev_tx_t e1000_xmit_frame(struct sk_buff *skb, struct net_device *netdev);

// references from netdev.c patch into (included) entl_device.c
static void entl_device_init(entl_device_t *dev);
static void entl_device_link_down(entl_device_t *dev);
static void entl_device_link_up(entl_device_t *dev);
static bool entl_device_process_rx_packet(entl_device_t *dev, struct sk_buff *skb);
static void entl_device_process_tx_packet(entl_device_t *dev, struct sk_buff *skb);
static int entl_do_ioctl(struct net_device *netdev, struct ifreq *ifr, int cmd);
static void entl_e1000_configure(struct e1000_adapter *adapter);
static void entl_e1000_set_my_addr(struct e1000_adapter *adapter, const uint8_t *addr);
#ifdef ENTL_TX_ON_ENTL_ENABLE
static netdev_tx_t entl_tx_transmit(struct sk_buff *skb, struct net_device *netdev);
#endif

// dead code
// static int entl_tx_queue_has_data(entl_device_t *dev);
// static void entl_tx_pull(struct net_device *netdev);

#endif
#endif
