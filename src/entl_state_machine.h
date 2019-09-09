#ifndef _ENTL_STATE_MACHINE_H_
#define _ENTL_STATE_MACHINE_H_

#define ENTL_ACTION_NOP      0x00
#define ENTL_ACTION_SEND     0x01
#define ENTL_ACTION_SEND_AIT 0x02
#define ENTL_ACTION_PROC_AIT 0x04
#define ENTL_ACTION_SIG_AIT  0x08
#define ENTL_ACTION_SEND_DAT 0x10
#define ENTL_ACTION_SIG_ERR  0x20
#define ENTL_ACTION_ERROR    -1

#define ENTL_MESSAGE_HELLO_U 0x0000
#define ENTL_MESSAGE_HELLO_L 0x00000000
#define ENTL_MESSAGE_EVENT_U 0x0001
#define ENTL_MESSAGE_NOP_U   0x0002
#define ENTL_MESSAGE_AIT_U   0x0003
#define ENTL_MESSAGE_ACK_U   0x0004
#define ENTL_MESSAGE_MASK    0x00ff
#define ENTL_MESSAGE_ONLY_U  0x8000
#define ENTL_TEST_MASK       0x7f00

static inline int get_entl_msg(uint16_t u_daddr) { return u_daddr & ENTL_MESSAGE_MASK; }

#define ENTL_STATE_IDLE     0
#define ENTL_STATE_HELLO    1
#define ENTL_STATE_WAIT     2
#define ENTL_STATE_SEND     3
#define ENTL_STATE_RECEIVE  4
#define ENTL_STATE_AM       5
#define ENTL_STATE_BM       6
#define ENTL_STATE_AH       7
#define ENTL_STATE_BH       8
#define ENTL_STATE_ERROR    9

// uint32_t entl_state = FETCH_STATE(p);

#include "entt_queue.h"
#include "entl_state.h"

#define ENTL_COUNT_MAX 10
#define ENTL_DEVICE_NAME_LEN 15
typedef struct entl_state_machine {
    spinlock_t state_lock;
    uint32_t state_count;
    entl_state_t current_state;
    entl_state_t error_state;
    entl_state_t return_state; // unused
    int user_pid;
    struct entt_ioctl_ait_data *receive_buffer;
    ENTT_queue_t send_ATI_queue;
    ENTT_queue_t receive_ATI_queue;
    char name[ENTL_DEVICE_NAME_LEN];

    uint16_t mac_hi; // MAC addr for Hello message
    uint32_t mac_lo; // MAC addr for Hello message
    uint8_t mac_valid;
    uint16_t hello_hi;
    uint32_t hello_lo;
    uint8_t hello_valid;
} entl_state_machine_t;

// FIXME: fields should have prefix (i.e. esm_)
static inline char *get_esm_name(entl_state_machine_t *mcn) { return mcn->name; }

static inline void set_update_time(entl_state_machine_t *mcn, struct timespec ts) { memcpy(&mcn->current_state.update_time, &ts, sizeof(struct timespec)); }
static inline int get_atomic_state(entl_state_machine_t *mcn) { return mcn->current_state.current_state; }
static inline void set_atomic_state(entl_state_machine_t *mcn, int value) { mcn->current_state.current_state = value; }
static inline int get_i_know(entl_state_machine_t *mcn) { return mcn->current_state.event_i_know; }
static inline void set_i_know(entl_state_machine_t *mcn, int value) { mcn->current_state.event_i_know = value; }
static inline int get_send_next(entl_state_machine_t *mcn) { return mcn->current_state.event_send_next; }
static inline void set_send_next(entl_state_machine_t *mcn, int value) { mcn->current_state.event_send_next = value; }
static inline void advance_send_next(entl_state_machine_t *mcn) { mcn->current_state.event_send_next += 2; }
static inline int get_i_sent(entl_state_machine_t *mcn) { return mcn->current_state.event_i_sent; }
static inline void set_i_sent(entl_state_machine_t *mcn, int value) { mcn->current_state.event_i_sent = value; }

static inline void zebra(entl_state_machine_t *mcn) { set_i_sent(mcn, get_send_next(mcn)); }

static inline void unicorn(entl_state_machine_t *mcn, int value) {
    // when following 3 members are all zero, it means fresh out of Hello handshake
    set_i_know(mcn, 0);
    set_send_next(mcn, 0);
    set_i_sent(mcn, 0);
    set_atomic_state(mcn, value);
}

static inline void clear_intervals(entl_state_machine_t *mcn) {
#ifdef ENTL_SPEED_CHECK
    memset(&mcn->current_state.interval_time, 0, sizeof(struct timespec));
    memset(&mcn->current_state.max_interval_time, 0, sizeof(struct timespec));
    memset(&mcn->current_state.min_interval_time, 0, sizeof(struct timespec));
#endif
}

static inline int current_error_pending(entl_state_machine_t *mcn) {
    return mcn->error_state.error_count;
}

static inline void clear_error(entl_state_machine_t *mcn) {
    mcn->current_state.error_flag = 0;
    mcn->current_state.error_count = 0;
}

static inline int recvq_count(entl_state_machine_t *mcn) { return mcn->receive_ATI_queue.count; }
static inline int recvq_full(entl_state_machine_t *mcn) { return ENTT_queue_full(&mcn->receive_ATI_queue); }
static inline void* recvq_pop(entl_state_machine_t *mcn) { return ENTT_queue_front_pop(&mcn->receive_ATI_queue); }
static inline int recvq_push(entl_state_machine_t *mcn) { return ENTT_queue_back_push(&mcn->receive_ATI_queue, mcn->receive_buffer); }

static inline int sendq_count(entl_state_machine_t *mcn) { return mcn->send_ATI_queue.count; }
static inline void* sendq_peek(entl_state_machine_t *mcn) { return ENTT_queue_front(&mcn->send_ATI_queue); }
static inline void* sendq_pop(entl_state_machine_t *mcn) { return ENTT_queue_front_pop(&mcn->send_ATI_queue); }
static inline int sendq_push(entl_state_machine_t *mcn, void *data) { return ENTT_queue_back_push(&mcn->send_ATI_queue, data); }

// when the 3 members (event_i_sent, event_i_know, event_send_next) are all zero, things are fresh out of Hello handshake
static inline void entl_state_machine_init(entl_state_machine_t *mcn) {
    mcn->state_count = 0;
    // current_state
        set_i_know(mcn, 0);
        set_i_sent(mcn, 0);
        set_send_next(mcn, 0);
        set_atomic_state(mcn, 0);
        clear_error(mcn); // error_flag, error_count
        mcn->current_state.p_error_flag = 0;
        memset(&mcn->current_state.update_time, 0, sizeof(struct timespec));
        memset(&mcn->current_state.error_time, 0, sizeof(struct timespec));
        clear_intervals(mcn); //  interval_time, max_interval_time, min_interval_time
    // error_state
    mcn->error_state.current_state = 0;
    mcn->error_state.error_flag = 0;
    // return_state
    mcn->user_pid = 0;
    // AIT mesage handling
    mcn->receive_buffer = NULL;
    ENTT_queue_init(&mcn->send_ATI_queue);
    ENTT_queue_init(&mcn->receive_ATI_queue);
    // hello
    mcn->mac_valid = 0;
    mcn->hello_valid = 0;
    spin_lock_init(&mcn->state_lock);
}

// Record first error state, acculate error flags
static inline void set_error(entl_state_machine_t *mcn, uint32_t error_flag) {
    entl_state_t *ep = &mcn->error_state;

    ep->error_count++;

    // FIXME: assumes we never wrap
    if (ep->error_count > 1) {
        ep->p_error_flag |= error_flag;
        return;
    }

    struct timespec ts = current_kernel_time();
    ep->event_i_know = get_i_know(mcn);
    ep->event_i_sent = get_i_sent(mcn);
    ep->current_state = get_atomic_state(mcn);
    ep->error_flag = error_flag;
    memcpy(&ep->update_time, &mcn->current_state.update_time, sizeof(struct timespec));
    memcpy(&ep->error_time, &ts, sizeof(struct timespec));
}

static inline void calc_intervals(entl_state_machine_t *mcn) {
#ifdef ENTL_SPEED_CHECK
    entl_state_t *cs = &mcn->current_state;
    struct timespec *ts_update = &cs->update_time;

    if (ts_update->tv_sec > 0 || ts_update->tv_nsec > 0) {
        struct timespec now = current_kernel_time();
        struct timespec *duration = &cs->interval_time;
        *duration = timespec_sub(*now - *ts_update);

        struct timespec *ts_max = &cs->max_interval_time;
        if (timespec_compare(ts_max, duration) < 0) {
            *ts_max = *duration;
        }

        struct timespec *ts_min = &cs->min_interval_time;
        if ((ts_min->tv_sec == 0 && ts_min->tv_nsec == 0)
        ||  (timespec_compare(duration, ts_min) < 0)) {
            *ts_min = *duration;
        }
    }
#endif
}

#endif
