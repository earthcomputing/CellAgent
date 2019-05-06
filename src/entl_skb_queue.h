#ifndef _ENTL_SKB_QUEUE_H_
#define _ENTL_SKB_QUEUE_H_

// ref: xx
#define ENTL_DEFAULT_TXD   256
typedef struct ENTL_skb_queue {
    uint16_t size;
    uint16_t count;
    uint16_t head;
    uint16_t tail;
    struct sk_buff *data[ENTL_DEFAULT_TXD];
} ENTL_skb_queue_t ;

// FIXME: add these as inline methods
static void init_ENTL_skb_queue(ENTL_skb_queue_t *q);
static int ENTL_skb_queue_full(ENTL_skb_queue_t *q);
static int ENTL_skb_queue_has_data(ENTL_skb_queue_t *q);
static int ENTL_skb_queue_unused(ENTL_skb_queue_t *q);
static struct sk_buff *front_ENTL_skb_queue(ENTL_skb_queue_t *q);
static struct sk_buff *pop_front_ENTL_skb_queue(ENTL_skb_queue_t *q);
static int push_back_ENTL_skb_queue(ENTL_skb_queue_t *q, struct sk_buff *dt);

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

#endif
