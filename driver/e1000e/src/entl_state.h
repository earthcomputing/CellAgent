#ifndef _ENTL_STATE_H_
#define _ENTL_STATE_H_

typedef struct entl_state {
    uint32_t event_i_know;       // last event received
    uint32_t event_i_sent;       // last event sent
    uint32_t event_send_next;    // next event sent
    uint32_t current_state;      // 0:idle 1:H 2:W 3:S 4:R
    struct timespec update_time; // last updated (usec)
    uint32_t error_flag;         // first error
    uint32_t p_error_flag;       // when multiple, union of error bits
    uint32_t error_count;        // multiple errors
    struct timespec error_time;  // first error detected (usec)
#ifdef ENTL_SPEED_CHECK
    struct timespec interval_time; // duration between S <-> R transition
    struct timespec max_interval_time;
    struct timespec min_interval_time;
#endif
} entl_state_t;

#endif
