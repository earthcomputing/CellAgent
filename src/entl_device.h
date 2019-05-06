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


// ref: xx
#define ENTL_DEFAULT_TXD   256
typedef struct ENTL_skb_queue {
    u16 size;
    u16 count;
    u16 head;
    u16 tail;
    struct sk_buff *data[ENTL_DEFAULT_TXD];
} ENTL_skb_queue_t ;

// #include "entl_skb_queue.h"
static void init_ENTL_skb_queue(ENTL_skb_queue_t *q);
static int ENTL_skb_queue_full(ENTL_skb_queue_t *q);
static int ENTL_skb_queue_has_data(ENTL_skb_queue_t *q);
static int ENTL_skb_queue_unused(ENTL_skb_queue_t *q);
static struct sk_buff *front_ENTL_skb_queue(ENTL_skb_queue_t *q);
static struct sk_buff *pop_front_ENTL_skb_queue(ENTL_skb_queue_t *q);
static int push_back_ENTL_skb_queue(ENTL_skb_queue_t *q, struct sk_buff *dt);

// FIXME: add inline methods
#if 0
static void init_ENTL_skb_queue(ENTL_skb_queue_t *q) {
    q->size = E1000_DEFAULT_TXD;
    q->count = 0;
    q->head = q->tail = 0;
}

static int ENTL_skb_queue_full(ENTL_skb_queue_t *q) { return (q->size == q->count) ? 1 : 0 }
static int ENTL_skb_queue_has_data(ENTL_skb_queue_t *q) { return q->count; }
static int ENTL_skb_queue_unused(ENTL_skb_queue_t *q) { return q->size - q->count - 1; }

static struct sk_buff *front_ENTL_skb_queue(ENTL_skb_queue_t *q) { return (q->count == 0) ? NULL : q->data[q->head]; }
static struct sk_buff *pop_front_ENTL_skb_queue(ENTL_skb_queue_t *q) {
    if (q->count == 0) return NULL;
    struct sk_buff *dt = q->data[q->head];
    q->head = (q->head + 1) % q->size;
    q->count--;
    return dt;
}

static int push_back_ENTL_skb_queue(ENTL_skb_queue_t *q, struct sk_buff *dt) {
    if (q->size == q->count) return -1;
    q->data[q->tail] = dt;
    q->tail = (q->tail+1) % q->size;
    q->count++;
    return q->size - q->count;
}
#endif

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
