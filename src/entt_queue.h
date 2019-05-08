#ifndef _ENTT_QUEUE_H_
#define _ENTT_QUEUE_H_

#define MAX_ENTT_QUEUE_SIZE 32
typedef struct ENTT_queue {
    uint16_t size;
    uint16_t count;
    uint16_t head;
    uint16_t tail;
    void *data[MAX_ENTT_QUEUE_SIZE];
} ENTT_queue_t;

static inline void ENTT_queue_init(ENTT_queue_t *q) {
    q->size = MAX_ENTT_QUEUE_SIZE;
    q->count = 0;
    q->head = q->tail = 0;
}

static inline int ENTT_queue_full(ENTT_queue_t *q) {
    return (q->size == q->count) ? 1 : 0;
}

static inline int ENTT_queue_back_push(ENTT_queue_t *q, void *dt) {
    if (q->size == q->count) return -1;
    q->data[q->tail] = dt;
    q->tail = (q->tail + 1) % q->size;
    q->count++;
    return q->size - q->count;
}

static inline void *ENTT_queue_front(ENTT_queue_t *q) {
    if (q->count == 0) return NULL;
    void *dt = q->data[q->head];
    return dt;
}

static inline void *ENTT_queue_front_pop(ENTT_queue_t *q) {
    if (q->count == 0) return NULL;
    void *dt = q->data[q->head];
    q->head = (q->head + 1) % q->size;
    q->count--;
    return dt;
}

#endif
