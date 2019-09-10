#include <linux/types.h>
#include <linux/string.h>
#include <linux/module.h>
#include <linux/slab.h>

#include "entl_state_machine.h"
#include "entt_queue.h"
#include "entl_stm_if.h"
#include "entl_user_api.h"

// newline should be unnecessary here - https://lwn.net/Articles/732420/
#define MCN_DEBUG(_name, _time, fmt, args...) printk(KERN_ALERT "%ld STM: %s " fmt "\n", _time, _name, ## args)
#define STM_TDEBUG(fmt, args...) MCN_DEBUG(mcn->name, ts.tv_sec, fmt, ## args)
#define STM_TDEBUG_ERROR(mcn, fmt, args...) STM_TDEBUG("error pending: flag %d (%s) count %d " fmt, mcn->error_state.error_flag, mcn_flag2name(mcn->error_state.error_flag), mcn->error_state.error_count, ## args)
// FIXME: add STM_TDEBUG_STATE, STM_TDEBUG_TRANSITION

#define STM_LOCK unsigned long flags; spin_lock_irqsave(&mcn->state_lock, flags)
#define STM_UNLOCK spin_unlock_irqrestore(&mcn->state_lock, flags)
#define OOPS_STM_UNLOCK spin_unlock(&mcn->state_lock)

#define respond_with(hi, lo, action) { *emsg_raw = hi; *seqno = lo; ret_action = action; }

static inline int cmp_addr(uint16_t l_high, uint32_t l_low, uint16_t r_high, uint32_t r_low) {
    if (l_high > r_high) return 1;
    if (l_high < r_high) return -1;
    return l_low - r_low;
}

void entl_set_my_adder(entl_state_machine_t *mcn, uint16_t mac_hi, uint32_t mac_lo) {
    struct timespec ts = current_kernel_time();
    STM_TDEBUG("set-id - macaddr %04x %08x", mac_hi, mac_lo); // FIXME: mcn name not set up ??
    STM_LOCK;
        mcn->mac_hi = mac_hi;
        mcn->mac_lo = mac_lo;
        mcn->mac_valid = 1;
        mcn->hello_valid = 0;
    STM_UNLOCK;
}

// unused ??
uint32_t get_entl_state(entl_state_machine_t *mcn) {
    STM_LOCK;
        uint16_t ret_state = (current_error_pending(mcn)) ? ENTL_STATE_ERROR : get_atomic_state(mcn);
    STM_UNLOCK; // OOPS_STM_UNLOCK;
    return ret_state;
}

// https://www.kernel.org/doc/html/latest/core-api/printk-formats.html
static char *error_bits[] = {
    "SEQUENCE",      // 0x0001 1 << 0
    "LINKDONW",      // 0x0002 1 << 1
    "TIMEOUT",       // 0x0004 1 << 2
    "SAME_ADDRESS",  // 0x0008 1 << 3
    "UNKOWN_CMD",    // 0x0010 1 << 4
    "UNKOWN_STATE",  // 0x0020 1 << 5
    "UNEXPECTED_LU", // 0x0040 1 << 6
    "FATAL"          // 0x8000 1 << 15
};

static inline char *mcn_flag2name(uint32_t s) {
    for (int i = 0; i < 7; i++) {
        if (s == (1 << i)) return error_bits[i];
    }
    if (s == 0x8000) return error_bits[7];
    return "??";
}

static char *mcn_names[] = {
    "IDLE",     // 0
    "HELLO",    // 1
    "WAIT",     // 2
    "SEND",     // 3
    "RECEIVE",  // 4
    "AM",       // 5
    "BM",       // 6
    "AH",       // 7
    "BH",       // 8
    "ERROR"     // 9
};

static inline char *mcn_state2name(uint32_t s) {
    return (s < 10) ? mcn_names[s] : "??";
}

// FIXME: emsg_op(emsg_type)
static inline char *msg_nick(int emsg_type) { return (emsg_type == ENTL_MESSAGE_EVENT_U) ? "EVENT" : (emsg_type == ENTL_MESSAGE_ACK_U) ? "ACK" : "??"; }

#define seqno_error(mcn, _action) { set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE); unicorn(mcn, ENTL_STATE_HELLO); set_update_time(mcn, ts); ret_action = _action;  }

// behavior : get_atomic_state(mcn) X emsg_type
int entl_received(entl_state_machine_t *mcn, uint16_t from_hi, uint32_t from_lo, uint16_t emsg_raw, uint32_t seqno) {
    struct timespec ts = current_kernel_time();
    uint16_t emsg_type = get_entl_msg(emsg_raw);

    if (emsg_type == ENTL_MESSAGE_NOP_U) return ENTL_ACTION_NOP;

    if (mcn->mac_valid == 0) {
        STM_TDEBUG("invalid, macaddr %04x %08x", mcn->mac_hi, mcn->mac_lo);
        return ENTL_ACTION_NOP;
    }

    if (current_error_pending(mcn)) {
        uint32_t was_state = get_atomic_state(mcn); // cheat - no locking
        STM_TDEBUG_ERROR(mcn, "message 0x%04x (%s) neighbor %04x %08x seqno %d, %s", emsg_raw, msg_nick(emsg_type), from_hi, from_lo, seqno, mcn_state2name(was_state));
        return ENTL_ACTION_SIG_ERR;
    }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        uint32_t was_state = get_atomic_state(mcn);
        switch (was_state) {
        case ENTL_STATE_IDLE: {
            STM_TDEBUG("message 0x%04x (%s) neighbor %04x %08x seqno %d, %s", emsg_raw, msg_nick(emsg_type), from_hi, from_lo, seqno, mcn_state2name(was_state));
        }
        ret_action = ENTL_ACTION_NOP;
        break;

        case ENTL_STATE_HELLO: {
            if (emsg_type == ENTL_MESSAGE_HELLO_U) {
                // establish neighbor identity:
                mcn->hello_hi = from_hi;
                mcn->hello_lo = from_lo;
                mcn->hello_valid = 1;

                STM_TDEBUG("neighbor %04x %08x", from_hi, from_lo);

                // symmetry breaking : master / slave
                int ordering = cmp_addr(mcn->mac_hi, mcn->mac_lo, from_hi, from_lo);
                if (ordering > 0) {
                // if ((mcn->mac_hi > from_hi) ||  ((mcn->mac_hi == from_hi) && (mcn->mac_lo > from_lo))) { // }
                    STM_TDEBUG("%s (master) -> WAIT", mcn_state2name(was_state));
                    unicorn(mcn, ENTL_STATE_WAIT); set_update_time(mcn, ts);
                    clear_intervals(mcn);
                    mcn->state_count = 0;
                    ret_action = ENTL_ACTION_SEND;
                }
                else if (ordering == 0) {
                // else if ((mcn->mac_hi == from_hi) && (mcn->mac_lo == from_lo)) { // }
                    // say error as Alan's 1990s problem again
                    STM_TDEBUG("Fatal Error - SAME ADDRESS, %s -> IDLE", mcn_state2name(was_state));
                    set_error(mcn, ENTL_ERROR_SAME_ADDRESS);
                    set_atomic_state(mcn, ENTL_STATE_IDLE);
                    set_update_time(mcn, ts);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("%s (slave)", mcn_state2name(was_state));
                    ret_action = ENTL_ACTION_NOP;
                }
            }
            else if (emsg_type == ENTL_MESSAGE_EVENT_U) {
                if (seqno != 0) {
                    STM_TDEBUG("EVENT: Out of Sequence: seqno %d, %s", seqno, mcn_state2name(was_state));
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("EVENT: advance - seqno %d, %s (slave) -> SEND", seqno, mcn_state2name(was_state));
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    calc_intervals(mcn);
                    set_update_time(mcn, ts);
                    ret_action = ENTL_ACTION_SEND;
                }
            }
            else {
                STM_TDEBUG("message 0x%04x (%s) neighbor %04x %08x seqno %d, %s", emsg_raw, msg_nick(emsg_type), from_hi, from_lo, seqno, mcn_state2name(was_state));
                // FIXME: dump whole packet here?
                ret_action = ENTL_ACTION_NOP;
            }
        }
        break;

        case ENTL_STATE_WAIT: {
            if (emsg_type == ENTL_MESSAGE_HELLO_U) {
                mcn->state_count++; // private to this logic
                if (mcn->state_count > ENTL_COUNT_MAX) {
                    STM_TDEBUG("overflow %d, %s -> HELLO", mcn->state_count, mcn_state2name(was_state));
                    unicorn(mcn, ENTL_STATE_HELLO); set_update_time(mcn, ts);
                }
                ret_action = ENTL_ACTION_NOP;
            }
            else if (emsg_type == ENTL_MESSAGE_EVENT_U) {
                // hmm, this should be exactly 1, not sent+1
                if (seqno == get_i_sent(mcn) + 1) {
                    STM_TDEBUG("EVENT: advance - seqno %d, %s (master) -> SEND", seqno, mcn_state2name(was_state));
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    set_update_time(mcn, ts);
                    clear_intervals(mcn);
                    ret_action = ENTL_ACTION_SEND;
                }
                else {
                    STM_TDEBUG("EVENT: wrong seqno %d, %s -> HELLO", seqno, mcn_state2name(was_state));
                    unicorn(mcn, ENTL_STATE_HELLO); set_update_time(mcn, ts);
                    clear_intervals(mcn);
                    ret_action = ENTL_ACTION_NOP;
                }
            }
            else {
                // Received non hello message on Wait state
                STM_TDEBUG("wrong message 0x%04x, %s -> HELLO", emsg_raw, mcn_state2name(was_state));
                seqno_error(mcn, ENTL_ACTION_NOP);
            }
        }
        break;

// FIXME: study this ??
        case ENTL_STATE_SEND: {
            if (emsg_type == ENTL_MESSAGE_EVENT_U
            ||  emsg_type == ENTL_MESSAGE_ACK_U) {
                if (seqno == get_i_know(mcn)) {
                    STM_TDEBUG("%s same seqno %d, SEND", msg_nick(emsg_type), seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("%s Out of Sequence: seqno %d, %s -> HELLO", msg_nick(emsg_type), seqno, mcn_state2name(was_state));
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else {
                STM_TDEBUG("wrong message 0x%04x, %s -> HELLO", emsg_raw, mcn_state2name(was_state));
                seqno_error(mcn, ENTL_ACTION_ERROR);
            }
        }
        break;

        case ENTL_STATE_RECEIVE: {
            if (emsg_type == ENTL_MESSAGE_EVENT_U) {
                if (get_i_know(mcn) + 2 == seqno) {
                    // FIXME : too frequent to log
                    // STM_TDEBUG("EVENT: advance - seqno %d, %s -> SEND", seqno, mcn_state2name(was_state));
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    ret_action = ENTL_ACTION_SEND;

                    // send queue non-empty
                    if (!sendq_count(mcn)) { // AIT has priority
                        STM_TDEBUG("EVENT: advance - seqno %d, %s -> SEND (data)", seqno, mcn_state2name(was_state));
                        ret_action |= ENTL_ACTION_SEND_DAT; // data send as optional
                    }
                    set_update_time(mcn, ts);
                }
                else if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("EVENT: unchanged - seqno %d, %s", seqno, mcn_state2name(was_state));
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("EVENT: Out of Sequence - seqno %d, %s -> HELLO", seqno, mcn_state2name(was_state));
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else if (emsg_type == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) + 2 == seqno) {
                    // FIXME : too frequent to log
                    // STM_TDEBUG("EVENT: advance - seqno %d, %s -> AH", seqno, mcn_state2name(was_state));
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_AH);
                    ret_action = ENTL_ACTION_PROC_AIT;

                    // recv queue space avail
                    if (!recvq_full(mcn)) {
                        STM_TDEBUG("AIT: advance - seqno %d, %s -> AH (data)", seqno, mcn_state2name(was_state));
                        ret_action |= ENTL_ACTION_SEND;
                    }
                    else {
                        STM_TDEBUG("AIT: queue full - seqno %d, %s -> AH (hold)", seqno, mcn_state2name(was_state));
                    }
                    set_update_time(mcn, ts);
                }
                else if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("AIT: unchanged - seqno %d, %s", seqno, mcn_state2name(was_state));
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("AIT: Out of Sequence - seqno %d, %s -> HELLO", seqno, mcn_state2name(was_state));
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else {
                STM_TDEBUG("wrong message 0x%04x, %s -> HELLO", emsg_raw, mcn_state2name(was_state));
                seqno_error(mcn, ENTL_ACTION_ERROR);
            }
        }
        break;

        // AIT message sent, waiting for ack
        case ENTL_STATE_AM: {
            if (emsg_type == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) + 2 == seqno) {
                    STM_TDEBUG("ACK: advance - seqno %d, %s -> BM", seqno, mcn_state2name(was_state));
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_BM);
                    set_update_time(mcn, ts);
                    ret_action = ENTL_ACTION_SEND;
                }
                else {
                    STM_TDEBUG("ACK: Out of Sequence - seqno %d, %s -> HELLO", seqno, mcn_state2name(was_state));
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else if (emsg_type == ENTL_MESSAGE_EVENT_U) {
                if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("EVENT: unchanged - seqno %d, %s", seqno, mcn_state2name(was_state));
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    // FIXME: is this just a 'normal' Out of Sequence ??
                    STM_TDEBUG("EVENT: wrong message 0x%04x - seqno %d, %s -> HELLO", emsg_raw, seqno, mcn_state2name(was_state));
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else {
                STM_TDEBUG("wrong message 0x%04x, %s -> HELLO", emsg_raw, mcn_state2name(was_state));
                seqno_error(mcn, ENTL_ACTION_ERROR);
            }
        }
        break;

        // AIT sent, Ack received, sending Ack
        case ENTL_STATE_BM: {
            if (emsg_type == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("ACK: unchanged - seqno %d, %s", seqno, mcn_state2name(was_state));
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    // FIXME: is this just a 'normal' Out of Sequence ??
                    STM_TDEBUG("ACK: wrong message 0x%04x - seqno %d, %s -> HELLO", emsg_raw, seqno, mcn_state2name(was_state));
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else {
                STM_TDEBUG("wrong message 0x%04x, %s -> HELLO", emsg_raw, mcn_state2name(was_state));
                seqno_error(mcn, ENTL_ACTION_ERROR);
            }
        }
        break;

        // AIT message received, sending Ack
        case ENTL_STATE_AH: {
            if (emsg_type == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("AIT: unchanged - seqno %d, %s", seqno, mcn_state2name(was_state));
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("AIT: Out of Sequence - seqno %d, %s -> HELLO", seqno, mcn_state2name(was_state));
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else {
                STM_TDEBUG("wrong message 0x%04x, %s -> HELLO", emsg_raw, mcn_state2name(was_state));
                seqno_error(mcn, ENTL_ACTION_ERROR);
            }
        }
        break;
// bj wrong message xx(emsg_type)
// bj move emsg_type out of fmt

        // got AIT, Ack sent, waiting for ack
        case ENTL_STATE_BH: {
            if (emsg_type == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) + 2 == seqno) {
                    STM_TDEBUG("ACK: advance - seqno %d, %s -> SEND", seqno, mcn_state2name(was_state));
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    set_update_time(mcn, ts);
// add to recvq
// STM_TDEBUG("recvq_push");
{
    struct entt_ioctl_ait_data *dt = mcn->receive_buffer;
    dump_ait_data(mcn, "stm - recvq_push", dt);
}
                    int recv_space = recvq_push(mcn);
                    // FIXME: what about when q is full?
                    mcn->receive_buffer = NULL;
                    ret_action = ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT;
                }
                else {
                    STM_TDEBUG("ACK: Out of Sequence - seqno %d, %s -> HELLO", seqno, mcn_state2name(was_state));
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else if (emsg_type == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("AIT: unchanged - seqno %d, %s", seqno, mcn_state2name(was_state));
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("AIT: Out of Sequence - seqno %d, %s -> HELLO", seqno, mcn_state2name(was_state));
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else {
                STM_TDEBUG("wrong message 0x%04x, %s -> HELLO", emsg_raw, mcn_state2name(was_state));
                seqno_error(mcn, ENTL_ACTION_ERROR);
            }
        }
        break;

        default: {
            STM_TDEBUG("wrong state %d (%s) -> IDLE", was_state, mcn_state2name(was_state));
            set_error(mcn, ENTL_ERROR_UNKOWN_STATE);
            unicorn(mcn, ENTL_STATE_IDLE); set_update_time(mcn, ts);
        }
        ret_action = ENTL_ACTION_NOP;
        break;
    }
    STM_UNLOCK;
    return ret_action;
}

int entl_get_hello(entl_state_machine_t *mcn, uint16_t *emsg_raw, uint32_t *seqno) {
    struct timespec ts = current_kernel_time();

    if (current_error_pending(mcn)) {
        STM_TDEBUG_ERROR(mcn, "entl_get_hello");
        return ENTL_ACTION_NOP;
    }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        uint32_t was_state = get_atomic_state(mcn);
        switch (was_state) {
        case ENTL_STATE_HELLO:
            respond_with(ENTL_MESSAGE_HELLO_U, ENTL_MESSAGE_HELLO_L, ENTL_ACTION_SEND);
            // STM_TDEBUG("HELLO(out): %s", mcn_state2name(was_state));
        break;

        case ENTL_STATE_WAIT:
            respond_with(ENTL_MESSAGE_EVENT_U, 0, ENTL_ACTION_SEND);
            // STM_TDEBUG("EVENT(out): %s", mcn_state2name(was_state));
        break;

// bj - should we display : get_i_sent(mcn), i.e. seqno
        case ENTL_STATE_RECEIVE:
            respond_with(ENTL_MESSAGE_EVENT_U, get_i_sent(mcn), ENTL_ACTION_SEND);
            STM_TDEBUG("EVENT(out): %s", mcn_state2name(was_state));
        break;

        case ENTL_STATE_AM:
            respond_with(ENTL_MESSAGE_AIT_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SEND_AIT);
            STM_TDEBUG("AIT(out): %s", mcn_state2name(was_state));
        break;

        case ENTL_STATE_BH:
            // recv queue space avail
            if (!recvq_full(mcn)) {
                respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND);
                STM_TDEBUG("ACK(out): %s", mcn_state2name(was_state));
            }
            else {
                ret_action = ENTL_ACTION_NOP;
            }
        break;

// FIXME: what state?
        default:
            ret_action = ENTL_ACTION_NOP;
        break;
        }
    STM_UNLOCK;
    return ret_action;
}

int entl_next_send(entl_state_machine_t *mcn, uint16_t *emsg_raw, uint32_t *seqno) {
    struct timespec ts = current_kernel_time();

    if (current_error_pending(mcn)) {
        int ret_action;
        uint32_t was_state = get_atomic_state(mcn);
        respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        STM_TDEBUG_ERROR(mcn, "entl_next_send - state %d (%s)", was_state, mcn_state2name(was_state));
        return ret_action;
    }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        uint32_t was_state = get_atomic_state(mcn);
        switch (was_state) {
        case ENTL_STATE_IDLE:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            STM_TDEBUG("NOP(out): %s", mcn_state2name(was_state));
        break;

        case ENTL_STATE_HELLO:
            respond_with(ENTL_MESSAGE_HELLO_U, ENTL_MESSAGE_HELLO_L, ENTL_ACTION_SEND);
            STM_TDEBUG("HELLO(out): %s", mcn_state2name(was_state));
        break;

        case ENTL_STATE_WAIT:
            respond_with(ENTL_MESSAGE_EVENT_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("EVENT(out): %s", mcn_state2name(was_state));
        break;

        case ENTL_STATE_SEND: {
            uint32_t event_i_know = get_i_know(mcn); // last received event number
            uint32_t event_i_sent = get_i_sent(mcn);
            zebra(mcn); advance_send_next(mcn);
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            // Avoid sending AIT on first exchange, neighbor will be in Hello state
            // send queue non-empty
            if (event_i_know && event_i_sent && sendq_count(mcn)) {
                set_atomic_state(mcn, ENTL_STATE_AM);
                respond_with(ENTL_MESSAGE_AIT_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SEND_AIT);
                STM_TDEBUG("AIT(out): seqno %d, %s -> AM", *seqno, mcn_state2name(was_state));
            }
            else {
                set_atomic_state(mcn, ENTL_STATE_RECEIVE);
                respond_with(ENTL_MESSAGE_EVENT_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SEND_DAT); // data send as optional
                // STM_TDEBUG("EVENT(out): seqno %d, %s -> AM", *seqno, mcn_state2name(was_state));
            }
        }
        break;

        case ENTL_STATE_RECEIVE:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("NOP(out): %s", mcn_state2name(was_state));
        break;

        // AIT
        case ENTL_STATE_AM:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("NOP(out): %s", mcn_state2name(was_state));
        break;

        case ENTL_STATE_BM: {
            zebra(mcn); advance_send_next(mcn);
            respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT);
            STM_TDEBUG("ACK(out): seqno %d, %s -> RECEIVE", *seqno, mcn_state2name(was_state));
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            calc_intervals(mcn);
            set_update_time(mcn, ts);

            // discard off send queue
            STM_TDEBUG("sendq_pop");
            struct entt_ioctl_ait_data *ait_data = sendq_pop(mcn);
            if (ait_data) {
                kfree(ait_data);
            }
        }
        break;

        case ENTL_STATE_AH: {
            // recv queue space avail
            if (!recvq_full(mcn)) {
                zebra(mcn); advance_send_next(mcn);
                respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND);
                STM_TDEBUG("ACK(out): seqno %d, %s -> BH", *seqno, mcn_state2name(was_state));
                set_atomic_state(mcn, ENTL_STATE_BH);
                calc_intervals(mcn);
                set_update_time(mcn, ts);
            }
            else {
                respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
                // STM_TDEBUG("NOP(out): %s", mcn_state2name(was_state));
            }
        }
        break;

        case ENTL_STATE_BH:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("NOP(out): %s", mcn_state2name(was_state));
        break;

// FIXME: what state?
        default:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("NOP(out): %s", mcn_state2name(was_state));
        break;
        }
    STM_UNLOCK;
    return ret_action;
}

// For TX, it can't send AIT, so just keep ENTL state on Send state
int entl_next_send_tx(entl_state_machine_t *mcn, uint16_t *emsg_raw, uint32_t *seqno) {
    struct timespec ts = current_kernel_time();

    // might be offline(no carrier), or be newly online after offline ??
    if (current_error_pending(mcn)) {
        int ret_action;
        uint32_t was_state = get_atomic_state(mcn);
        respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        STM_TDEBUG_ERROR(mcn, "entl_next_send_tx - state %d (%s)", was_state, mcn_state2name(was_state));
        return ret_action;
    }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        uint32_t was_state = get_atomic_state(mcn);
        switch (was_state) {
        case ENTL_STATE_IDLE:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            STM_TDEBUG("NOP(out): %s", mcn_state2name(was_state));
        break;

        case ENTL_STATE_HELLO:
            respond_with(ENTL_MESSAGE_HELLO_U, ENTL_MESSAGE_HELLO_L, ENTL_ACTION_SEND);
            // STM_TDEBUG("HELLO(out): %s", mcn_state2name(was_state));
        break;

        case ENTL_STATE_WAIT:
            respond_with(ENTL_MESSAGE_EVENT_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("EVENT(out): %s", mcn_state2name(was_state));
        break;

        case ENTL_STATE_SEND: {
            zebra(mcn); advance_send_next(mcn);
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            respond_with(ENTL_MESSAGE_EVENT_U, get_i_sent(mcn), ENTL_ACTION_SEND);
            // STM_TDEBUG("EVENT(out): %s", mcn_state2name(was_state));
            // For TX, it can't send AIT, so just keep ENTL state on Send state
        }
        break;

        case ENTL_STATE_RECEIVE:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("NOP(out): %s", mcn_state2name(was_state));
        break;

        // AIT
        case ENTL_STATE_AM:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("NOP(out): %s", mcn_state2name(was_state));
        break;

        case ENTL_STATE_BM: {
            zebra(mcn); advance_send_next(mcn);
            respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT);
            STM_TDEBUG("ACK(out): seqno %d, %s -> RECEIVE", *seqno, mcn_state2name(was_state));
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            calc_intervals(mcn);
            set_update_time(mcn, ts);

            // discard off send queue
            STM_TDEBUG("sendq_pop");
            struct entt_ioctl_ait_data *ait_data = sendq_pop(mcn);
            // FIXME: memory leak?
        }
        break;

        case ENTL_STATE_AH: {
            // recv queue space avail
            if (!recvq_full(mcn)) {
                zebra(mcn); advance_send_next(mcn);
                respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND);
                STM_TDEBUG("ACK(out): seqno %d, %s -> BH", *seqno, mcn_state2name(was_state));
                set_atomic_state(mcn, ENTL_STATE_BH);
                calc_intervals(mcn);
                set_update_time(mcn, ts);
            }
            else {
                respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
                // STM_TDEBUG("NOP(out): %s", mcn_state2name(was_state));
            }
        }
        break;

        case ENTL_STATE_BH:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("NOP(out): %s", mcn_state2name(was_state));
        break;

// FIXME: what state?
        default:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("NOP(out): %s", mcn_state2name(was_state));
        break;
        }
    STM_UNLOCK;
    return ret_action;
}

void entl_state_error(entl_state_machine_t *mcn, uint32_t error_flag) {
    struct timespec ts = current_kernel_time();

    uint32_t was_state = get_atomic_state(mcn);
    if ((error_flag == ENTL_ERROR_FLAG_LINKDONW) && (was_state == ENTL_STATE_IDLE)) return;

    STM_LOCK;
        set_error(mcn, error_flag);
        if (error_flag == ENTL_ERROR_FLAG_LINKDONW) {
            set_atomic_state(mcn, ENTL_STATE_IDLE);
        }
        else if (error_flag == ENTL_ERROR_FLAG_SEQUENCE) {
// FIXME : seems redundant ?
            unicorn(mcn, ENTL_STATE_HELLO); set_update_time(mcn, ts);
            clear_error(mcn);
            clear_intervals(mcn);
        }
    STM_UNLOCK;
    uint32_t now = get_atomic_state(mcn);
    STM_TDEBUG("entl_state_error - flag %d (%s), was %d (%s), now %d (%s)", error_flag, mcn_flag2name(error_flag), was_state, mcn_state2name(was_state), now, mcn_state2name(now));
}

void entl_read_current_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err) {
    struct timespec ts = current_kernel_time();
    STM_LOCK;
        memcpy(st, &mcn->current_state, sizeof(entl_state_t));
        memcpy(err, &mcn->error_state, sizeof(entl_state_t));
    STM_UNLOCK;
}

void entl_read_error_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err) {
    struct timespec ts = current_kernel_time();
    STM_LOCK;
        memcpy(st, &mcn->current_state, sizeof(entl_state_t));
        memcpy(err, &mcn->error_state, sizeof(entl_state_t));
        memset(&mcn->error_state, 0, sizeof(entl_state_t));
    STM_UNLOCK;
    uint32_t was_state = st->current_state;
    uint32_t count = err->error_count;
    uint32_t error_flag = err->error_flag;
    uint32_t mask = err->p_error_flag;
    STM_TDEBUG("read-and-clear error_state -"
        " state %d (%s)"
        " flag 0x%04x (%s)"
        " count %d"
        " mask 0x%04x",
        was_state, mcn_state2name(was_state),
        error_flag, mcn_flag2name(error_flag),
        count,
        mask
    );
}

void entl_link_up(entl_state_machine_t *mcn) {
    struct timespec ts = current_kernel_time();
    STM_LOCK;
        uint32_t was_state = get_atomic_state(mcn);
        if (was_state != ENTL_STATE_IDLE) {
            STM_TDEBUG("Link Up, state %d (%s), unexpected, ignored", was_state, mcn_state2name(was_state));
        }
        else if (current_error_pending(mcn)) {
// FIXME: is error 'DOWN' ??
            STM_TDEBUG_ERROR(mcn, "Link Up, error lock - state %d (%s)", was_state, mcn_state2name(was_state));
        }
        else {
            STM_TDEBUG("Link Up, %s -> HELLO", mcn_state2name(was_state));
            unicorn(mcn, ENTL_STATE_HELLO); set_update_time(mcn, ts);
            clear_error(mcn);
            clear_intervals(mcn);
        }
    STM_UNLOCK;
}

// AIT handling functions
// add AIT message to send queue, return 0 when OK, -1 when queue full
int entl_send_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data) {
    struct timespec ts = current_kernel_time();
STM_TDEBUG("sendq_push");
    STM_LOCK;
        int send_space = sendq_push(mcn, (void *) data);
    STM_UNLOCK;
    STM_TDEBUG("sendq_push - space %d", send_space);
    return send_space;
}

// peek at next AIT message to xmit
struct entt_ioctl_ait_data *entl_next_AIT_message(entl_state_machine_t *mcn) {
    struct timespec ts = current_kernel_time();
STM_TDEBUG("sendq_peek");
    STM_LOCK;
        struct entt_ioctl_ait_data *dt = (struct entt_ioctl_ait_data *) sendq_peek(mcn);
    STM_UNLOCK;
    // if (dt) STM_TDEBUG("sendq_peek"); // counts
    return dt;
}

// new AIT message received
void entl_new_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data) {
    STM_LOCK;
        mcn->receive_buffer = data;
    STM_UNLOCK;
}

// Read (consume) AIT message, return NULL when queue empty
struct entt_ioctl_ait_data *entl_read_AIT_message(entl_state_machine_t *mcn) {
    struct timespec ts = current_kernel_time();
STM_TDEBUG("recvq_pop");
    STM_LOCK;
        struct entt_ioctl_ait_data *dt = recvq_pop(mcn);
        if (dt) {
            dt->num_messages = recvq_count(mcn);
            dt->num_queued = sendq_count(mcn);
            STM_TDEBUG("recvq_pop - msgs %d nqueued %d", dt->num_messages, dt->num_queued);
        }
// FIXME: should allocate and return an 'empty' dt w/counts
    STM_UNLOCK;
    return dt;
}

// number of pending xmits
uint16_t entl_num_queued(entl_state_machine_t *mcn) {
    STM_LOCK;
        uint16_t count = sendq_count(mcn);
    STM_UNLOCK;
    return count;
}
