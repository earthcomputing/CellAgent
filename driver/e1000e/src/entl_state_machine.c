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
        STM_TDEBUG("invalid macaddr %04x %08x", mcn->mac_hi, mcn->mac_lo);
        return ENTL_ACTION_NOP;
    }

    if (current_error_pending(mcn)) {
        uint32_t was_state = get_atomic_state(mcn); // cheat - no locking
        STM_TDEBUG_ERROR(mcn, "%s message %s (0x%04x) neighbor %04x %08x seqno %d", mcn_state2name(was_state), msg_nick(emsg_type), emsg_raw, from_hi, from_lo, seqno);
        return ENTL_ACTION_SIG_ERR;
    }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        uint32_t was_state = get_atomic_state(mcn);
        switch (was_state) {
        case ENTL_STATE_IDLE: {
            STM_TDEBUG("%s message %s (0x%04x) neighbor %04x %08x seqno %d", mcn_state2name(was_state), msg_nick(emsg_type), emsg_raw, from_hi, from_lo, seqno);
        }
        ret_action = ENTL_ACTION_NOP;
        break;

        case ENTL_STATE_HELLO: {
            if (emsg_type == ENTL_MESSAGE_HELLO_U) {
                // establish neighbor identity:
                mcn->hello_hi = from_hi;
                mcn->hello_lo = from_lo;
                mcn->hello_valid = 1;

                STM_TDEBUG("%04x %08x greeting - neighbor %04x %08x", mcn->mac_hi, mcn->mac_lo, from_hi, from_lo);

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
                    STM_TDEBUG("%s -> IDLE - Fatal Error: SAME ADDRESS", mcn_state2name(was_state));
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
                    STM_TDEBUG("%s EVENT(in): Out of Sequence - seqno %d", mcn_state2name(was_state), seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("%s (slave) -> SEND EVENT: advance - seqno %d", mcn_state2name(was_state), seqno);
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    calc_intervals(mcn);
                    set_update_time(mcn, ts);
                    ret_action = ENTL_ACTION_SEND;
                }
            }
            else {
                STM_TDEBUG("%s WTF? message %s (0x%04x) neighbor %04x %08x seqno %d (0x%08x)", mcn_state2name(was_state), msg_nick(emsg_type), emsg_raw, from_hi, from_lo, seqno, seqno);
                // FIXME: dump whole packet here?
                ret_action = ENTL_ACTION_NOP;
            }
        }
        break;

        case ENTL_STATE_WAIT: {
            if (emsg_type == ENTL_MESSAGE_HELLO_U) {
                mcn->state_count++; // private to this logic
                if (mcn->state_count > ENTL_COUNT_MAX) {
                    STM_TDEBUG("%s -> HELLO - overflow %d", mcn_state2name(was_state), mcn->state_count);
                    unicorn(mcn, ENTL_STATE_HELLO); set_update_time(mcn, ts);
                }
                ret_action = ENTL_ACTION_NOP;
            }
            else if (emsg_type == ENTL_MESSAGE_EVENT_U) {
                // hmm, this should be exactly 1, not sent+1
                if (seqno == get_i_sent(mcn) + 1) {
                    STM_TDEBUG("%s (master) -> SEND EVENT(in): advance - seqno %d", mcn_state2name(was_state), seqno);
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    set_update_time(mcn, ts);
                    clear_intervals(mcn);
                    ret_action = ENTL_ACTION_SEND;
                }
                else {
                    STM_TDEBUG("%s -> HELLO EVENT(in): wrong seqno %d", mcn_state2name(was_state), seqno);
                    unicorn(mcn, ENTL_STATE_HELLO); set_update_time(mcn, ts);
                    clear_intervals(mcn);
                    ret_action = ENTL_ACTION_NOP;
                }
            }
            else {
                // Received non hello message on Wait state
                STM_TDEBUG("%s -> HELLO wrong message 0x%04x", mcn_state2name(was_state), emsg_raw);
                seqno_error(mcn, ENTL_ACTION_NOP);
            }
        }
        break;

// FIXME: study this ??
        case ENTL_STATE_SEND: {
            if (emsg_type == ENTL_MESSAGE_EVENT_U
            ||  emsg_type == ENTL_MESSAGE_ACK_U) {
                if (seqno == get_i_know(mcn)) {
                    STM_TDEBUG("%s(in) same seqno %d, SEND", msg_nick(emsg_type), seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("%s -> HELLO %s(in): Out of Sequence - seqno %d", mcn_state2name(was_state), msg_nick(emsg_type), seqno);
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else {
                STM_TDEBUG("%s -> HELLO wrong message 0x%04x", mcn_state2name(was_state), emsg_raw);
                seqno_error(mcn, ENTL_ACTION_ERROR);
            }
        }
        break;

        case ENTL_STATE_RECEIVE: {
            if (emsg_type == ENTL_MESSAGE_EVENT_U) {
                if (get_i_know(mcn) + 2 == seqno) {
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    ret_action = ENTL_ACTION_SEND;

                    int pending = sendq_count(mcn);
                    // int nfree = sendq_space(mcn);
                    // int avail = recvq_space(mcn);
                    // int delivered = recvq_count(mcn);

                    // send queue empty
                    if (pending == 0) { // AIT has priority
                        // way too noisy to log!
                        // STM_TDEBUG("%s -> SEND (data) EVENT(in): advance - seqno %d", mcn_state2name(was_state), seqno);
                        ret_action |= ENTL_ACTION_SEND_DAT; // data send as optional
                    }
                    set_update_time(mcn, ts);
                }
                else if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("%s EVENT(in): unchanged - seqno %d", mcn_state2name(was_state), seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("%s -> HELLO EVENT(in): Out of Sequence - seqno %d", mcn_state2name(was_state), seqno);
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else if (emsg_type == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) + 2 == seqno) {
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_AH);
                    ret_action = ENTL_ACTION_PROC_AIT;

                    // int pending = sendq_count(mcn);
                    // int nfree = sendq_space(mcn);
                    int avail = recvq_space(mcn);
                    int delivered = recvq_count(mcn);
                    // recv queue space avail
                    if (avail > 0) {
                        STM_TDEBUG("%s -> AH (delivered %d avail %d) AIT(in): advance - seqno %d", mcn_state2name(was_state), delivered, avail, seqno);
                        ret_action |= ENTL_ACTION_SEND;
                    }
                    else {
                        STM_TDEBUG("%s -> AH (hold) AIT(in): queue full - seqno %d", mcn_state2name(was_state), seqno);
                    }
                    set_update_time(mcn, ts);
                }
                else if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("%s AIT(in): unchanged - seqno %d", mcn_state2name(was_state), seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("%s -> HELLO AIT(in): Out of Sequence - seqno %d", mcn_state2name(was_state), seqno);
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else {
                STM_TDEBUG("%s -> HELLO wrong message 0x%04x", mcn_state2name(was_state), emsg_raw);
                seqno_error(mcn, ENTL_ACTION_ERROR);
            }
        }
        break;

        // AIT message sent, waiting for ack
        case ENTL_STATE_AM: {
            if (emsg_type == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) + 2 == seqno) {
                    STM_TDEBUG("%s -> BM ACK(in): advance - seqno %d", mcn_state2name(was_state), seqno);
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_BM);
                    set_update_time(mcn, ts);
                    ret_action = ENTL_ACTION_SEND;
                }
                else {
                    STM_TDEBUG("%s -> HELLO ACK(in): Out of Sequence - seqno %d", mcn_state2name(was_state), seqno);
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else if (emsg_type == ENTL_MESSAGE_EVENT_U) {
                if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("%s EVENT(in): unchanged - seqno %d", mcn_state2name(was_state), seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    // FIXME: is this just a 'normal' Out of Sequence ??
                    STM_TDEBUG("%s -> HELLO EVENT(in): wrong message 0x%04x - seqno %d", mcn_state2name(was_state), emsg_raw, seqno);
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else {
                STM_TDEBUG("%s -> HELLO wrong message 0x%04x", mcn_state2name(was_state), emsg_raw);
                seqno_error(mcn, ENTL_ACTION_ERROR);
            }
        }
        break;

        // AIT sent, Ack received, sending Ack
        case ENTL_STATE_BM: {
            if (emsg_type == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("%s ACK(in): unchanged - seqno %d", mcn_state2name(was_state), seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    // FIXME: is this just a 'normal' Out of Sequence ??
                    STM_TDEBUG("%s -> HELLO ACK(in): wrong message 0x%04x - seqno %d", mcn_state2name(was_state), emsg_raw, seqno);
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else {
                STM_TDEBUG("%s -> HELLO wrong message 0x%04x", mcn_state2name(was_state), emsg_raw);
                seqno_error(mcn, ENTL_ACTION_ERROR);
            }
        }
        break;

        // AIT message received, sending Ack
        case ENTL_STATE_AH: {
            if (emsg_type == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("%s AIT(in): unchanged - seqno %d", mcn_state2name(was_state), seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("%s -> HELLO AIT(in): Out of Sequence - seqno %d", mcn_state2name(was_state), seqno);
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else {
                STM_TDEBUG("%s -> HELLO wrong message 0x%04x", mcn_state2name(was_state), emsg_raw);
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
                    STM_TDEBUG("%s -> SEND ACK(in): advance - seqno %d", mcn_state2name(was_state), seqno);
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    set_update_time(mcn, ts);
                    {
                        struct entt_ioctl_ait_data *dt = mcn->receive_buffer;
                        dump_ait_data(mcn, "stm - recvq_push", dt);
                    }

                    // int pending = sendq_count(mcn);
                    // int nfree = sendq_space(mcn);
                    int avail = recvq_space(mcn);
                    int delivered = recvq_count(mcn);

                    // FIXME: what about when q is full?
                    // add to recvq
                    int recv_space = recvq_push(mcn);
                    STM_TDEBUG("recvq_push - delivered %d avail: before %d after %d", delivered, avail, recv_space);
                    mcn->receive_buffer = NULL;
                    ret_action = ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT;
                }
                else {
                    STM_TDEBUG("%s -> HELLO ACK(in): Out of Sequence - seqno %d", mcn_state2name(was_state), seqno);
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else if (emsg_type == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("%s AIT(in): unchanged - seqno %d", mcn_state2name(was_state), seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("%s -> HELLO AIT(in): Out of Sequence - seqno %d", mcn_state2name(was_state), seqno);
                    seqno_error(mcn, ENTL_ACTION_ERROR);
                }
            }
            else {
                STM_TDEBUG("%s -> HELLO wrong message 0x%04x", mcn_state2name(was_state), emsg_raw);
                seqno_error(mcn, ENTL_ACTION_ERROR);
            }
        }
        break;

        default: {
            STM_TDEBUG("%s -> IDLE wrong state", mcn_state2name(was_state));
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
            // STM_TDEBUG("%s HELLO(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        case ENTL_STATE_WAIT:
            respond_with(ENTL_MESSAGE_EVENT_U, 0, ENTL_ACTION_SEND);
            // STM_TDEBUG("%s EVENT(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

// bj - should we display : get_i_sent(mcn), i.e. seqno
        case ENTL_STATE_RECEIVE:
            respond_with(ENTL_MESSAGE_EVENT_U, get_i_sent(mcn), ENTL_ACTION_SEND);
            STM_TDEBUG("%s EVENT(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        case ENTL_STATE_AM:
            respond_with(ENTL_MESSAGE_AIT_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SEND_AIT);
            STM_TDEBUG("%s AIT(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        case ENTL_STATE_BH: {
            // int pending = sendq_count(mcn);
            // int nfree = sendq_space(mcn);
            int avail = recvq_space(mcn);
            int delivered = recvq_count(mcn);
            // recv queue space avail
            if (avail > 0) {
                respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND);
                STM_TDEBUG("%s ACK(out) - delivered %d avail %d seqno %d", mcn_state2name(was_state), delivered, avail, *seqno);
            }
            else {
                ret_action = ENTL_ACTION_NOP;
            }
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
        STM_TDEBUG_ERROR(mcn, "%s entl_next_send", mcn_state2name(was_state));
        return ret_action;
    }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        uint32_t was_state = get_atomic_state(mcn);
        switch (was_state) {
        case ENTL_STATE_IDLE:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            STM_TDEBUG("%s NOP(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        case ENTL_STATE_HELLO:
            respond_with(ENTL_MESSAGE_HELLO_U, ENTL_MESSAGE_HELLO_L, ENTL_ACTION_SEND);
            STM_TDEBUG("%s HELLO(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        case ENTL_STATE_WAIT:
            respond_with(ENTL_MESSAGE_EVENT_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("%s EVENT(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        case ENTL_STATE_SEND: {
            uint32_t event_i_know = get_i_know(mcn); // last received event number
            uint32_t event_i_sent = get_i_sent(mcn);
            zebra(mcn); advance_send_next(mcn);
            calc_intervals(mcn);
            set_update_time(mcn, ts);

            int pending; // = sendq_count(mcn);
            int nfree = sendq_space(mcn);
            // int avail = recvq_space(mcn);
            // int delivered = recvq_count(mcn);
            // Avoid sending AIT on first exchange, neighbor will be in Hello state
            // send queue non-empty
            if (event_i_know && event_i_sent && (pending = sendq_count(mcn))) {
                set_atomic_state(mcn, ENTL_STATE_AM);
                respond_with(ENTL_MESSAGE_AIT_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SEND_AIT);
                STM_TDEBUG("%s -> AM AIT(out) - pending %d nfree %d seqno %d", mcn_state2name(was_state), pending, nfree, *seqno);
            }
            else {
                set_atomic_state(mcn, ENTL_STATE_RECEIVE);
                respond_with(ENTL_MESSAGE_EVENT_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SEND_DAT); // data send as optional
                // STM_TDEBUG("%s -> RECEIVE EVENT(out) - seqno %d", mcn_state2name(was_state), *seqno);
            }
        }
        break;

        case ENTL_STATE_RECEIVE:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("%s NOP(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        // AIT
        case ENTL_STATE_AM:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("%s NOP(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        case ENTL_STATE_BM: {
            zebra(mcn); advance_send_next(mcn);
            respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT);
            STM_TDEBUG("%s -> RECEIVE ACK(out) - seqno %d", mcn_state2name(was_state), *seqno);
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            calc_intervals(mcn);
            set_update_time(mcn, ts);

            // discard off send queue
            struct entt_ioctl_ait_data *ait_data = sendq_pop(mcn);
            if (ait_data) {
                // int pending = sendq_count(mcn);
                // int nfree = sendq_space(mcn);
                // int avail = recvq_space(mcn);
                // int delivered = recvq_count(mcn);
                int delivered = ait_data->num_messages;
                int pending = ait_data->num_queued;
                STM_TDEBUG("sendq_pop - pending %d", pending); //  recvq delivered %d", delivered);
                kfree(ait_data);
            }
            else {
                STM_TDEBUG("sendq_pop - empty");
            }
        }
        break;

        case ENTL_STATE_AH: {
            // int pending = sendq_count(mcn);
            // int nfree = sendq_space(mcn);
            int avail = recvq_space(mcn);
            int delivered = recvq_count(mcn);
            // recv queue space avail
            if (avail > 0) {
                zebra(mcn); advance_send_next(mcn);
                respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND);
                STM_TDEBUG("%s -> BH ACK(out) - delivered %d avail %d seqno %d", mcn_state2name(was_state), delivered, avail, *seqno);
                set_atomic_state(mcn, ENTL_STATE_BH);
                calc_intervals(mcn);
                set_update_time(mcn, ts);
            }
            else {
                respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
                // STM_TDEBUG("%s NOP(out) - seqno %d", mcn_state2name(was_state), *seqno);
            }
        }
        break;

        case ENTL_STATE_BH:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("%s NOP(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

// FIXME: what state?
        default:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("%s NOP(out) - seqno %d", mcn_state2name(was_state), *seqno);
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
        STM_TDEBUG_ERROR(mcn, "%s entl_next_send_tx", mcn_state2name(was_state));
        return ret_action;
    }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        uint32_t was_state = get_atomic_state(mcn);
        switch (was_state) {
        case ENTL_STATE_IDLE:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            STM_TDEBUG("%s NOP(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        case ENTL_STATE_HELLO:
            respond_with(ENTL_MESSAGE_HELLO_U, ENTL_MESSAGE_HELLO_L, ENTL_ACTION_SEND);
            // STM_TDEBUG("%s HELLO(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        case ENTL_STATE_WAIT:
            respond_with(ENTL_MESSAGE_EVENT_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("%s EVENT(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        case ENTL_STATE_SEND: {
            zebra(mcn); advance_send_next(mcn);
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            respond_with(ENTL_MESSAGE_EVENT_U, get_i_sent(mcn), ENTL_ACTION_SEND);
            // STM_TDEBUG("%s EVENT(out) - seqno %d", mcn_state2name(was_state), *seqno);
            // For TX, it can't send AIT, so just keep ENTL state on Send state
        }
        break;

        case ENTL_STATE_RECEIVE:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("%s NOP(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        // AIT
        case ENTL_STATE_AM:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("%s NOP(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

        case ENTL_STATE_BM: {
            zebra(mcn); advance_send_next(mcn);
            respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT);
            STM_TDEBUG("%s -> RECEIVE ACK(out) - seqno %d", mcn_state2name(was_state), *seqno);
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            calc_intervals(mcn);
            set_update_time(mcn, ts);

            // discard off send queue
            struct entt_ioctl_ait_data *ait_data = sendq_pop(mcn);
            if (ait_data) {
                // int pending = sendq_count(mcn);
                // int nfree = sendq_space(mcn);
                // int avail = recvq_space(mcn);
                // int delivered = recvq_count(mcn);
                int pending = ait_data->num_queued;
                int delivered = ait_data->num_messages;
                STM_TDEBUG("sendq_pop - pending %d", pending); //  recvq delivered %d", delivered);
                // FIXME: memory leak?
                // kfree(ait_data);
            }
            else {
                STM_TDEBUG("sendq_pop - empty");
            }
        }
        break;

        case ENTL_STATE_AH: {
            // int pending = sendq_count(mcn);
            // int nfree = sendq_space(mcn);
            int avail = recvq_space(mcn);
            int delivered = recvq_count(mcn);
            // recv queue space avail
            if (avail > 0) {
                zebra(mcn); advance_send_next(mcn);
                respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND);
                STM_TDEBUG("%s -> BH ACK(out) - delivered %d avail %d seqno %d", mcn_state2name(was_state), delivered, avail, *seqno);
                set_atomic_state(mcn, ENTL_STATE_BH);
                calc_intervals(mcn);
                set_update_time(mcn, ts);
            }
            else {
                respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
                // STM_TDEBUG("%s NOP(out) - seqno %d", mcn_state2name(was_state), *seqno);
            }
        }
        break;

        case ENTL_STATE_BH:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("%s NOP(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;

// FIXME: what state?
        default:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            // STM_TDEBUG("%s NOP(out) - seqno %d", mcn_state2name(was_state), *seqno);
        break;
        }
    STM_UNLOCK;
    return ret_action;
}

void entl_state_error(entl_state_machine_t *mcn, uint32_t error_flag) {
    struct timespec ts = current_kernel_time();

    uint32_t was_state = get_atomic_state(mcn);

    STM_LOCK;
        set_error(mcn, error_flag);
        if (error_flag == ENTL_ERROR_FLAG_SEQUENCE) {
            // FIXME : seems redundant ?
            unicorn(mcn, ENTL_STATE_HELLO); set_update_time(mcn, ts);
            clear_error(mcn);
            clear_intervals(mcn);
        }
        // FIXME: what about other values for error_flag ??
        uint32_t now = get_atomic_state(mcn);
    STM_UNLOCK;
    STM_TDEBUG("%s -> %s entl_state_error - flag %s (%d)", mcn_state2name(was_state), mcn_state2name(now), mcn_flag2name(error_flag), error_flag);
}

void entl_read_current_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err) {
    struct timespec ts = current_kernel_time();
    STM_LOCK;
        memcpy(st, &mcn->current_state, sizeof(entl_state_t));
        memcpy(err, &mcn->error_state, sizeof(entl_state_t));
    STM_UNLOCK;
}

void entl_clear_error_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err) {
    struct timespec ts = current_kernel_time();
    STM_LOCK;
        memset(&mcn->error_state, 0, sizeof(entl_state_t));
    STM_UNLOCK;
    uint32_t was_state = st->current_state;
    uint32_t count = err->error_count;
    uint32_t error_flag = err->error_flag;
    uint32_t mask = err->p_error_flag;
    STM_TDEBUG("state %s (%d) set error_state -"
        " flag %s (0x%04x)"
        " count %d"
        " mask 0x%04x",
        mcn_state2name(was_state), was_state,
        mcn_flag2name(error_flag), error_flag,
        count,
        mask
    );
}

void entl_read_error_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err) {
    struct timespec ts = current_kernel_time();
    STM_LOCK;
        memcpy(st, &mcn->current_state, sizeof(entl_state_t));
        memcpy(err, &mcn->error_state, sizeof(entl_state_t));
    STM_UNLOCK;
    uint32_t was_state = st->current_state;
    uint32_t count = err->error_count;
    uint32_t error_flag = err->error_flag;
    uint32_t mask = err->p_error_flag;
    STM_TDEBUG("state %s (%d) read error_state -"
        " flag %s (0x%04x)"
        " count %d"
        " mask 0x%04x",
        mcn_state2name(was_state), was_state,
        mcn_flag2name(error_flag), error_flag,
        count,
        mask
    );
}

void entl_link_up(entl_state_machine_t *mcn) {
    struct timespec ts = current_kernel_time();
    STM_LOCK;
        uint32_t was_state = get_atomic_state(mcn);
        if (was_state != ENTL_STATE_IDLE) {
            STM_TDEBUG("%s - Link Up unexpected, ignored", mcn_state2name(was_state));
        }
        else if (current_error_pending(mcn)) {
            // FIXME: is error 'LINKDONW' ??
            STM_TDEBUG_ERROR(mcn, "%s - Link Up, error lock", mcn_state2name(was_state));
        }
        else {
            STM_TDEBUG("%s -> HELLO - Link Up", mcn_state2name(was_state));
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
    STM_LOCK;
        int nfree = sendq_push(mcn, (void *) data); // sendq_space, after
    STM_UNLOCK;
    int pending = sendq_count(mcn);
    // int nfree = sendq_space(mcn);
    int avail = recvq_space(mcn);
    // int delivered = recvq_count(mcn);
    STM_TDEBUG("sendq_push - pending %d nfree %d", pending, nfree);
    return nfree;
}

// peek at next AIT message to xmit
struct entt_ioctl_ait_data *entl_next_AIT_message(entl_state_machine_t *mcn) {
    struct timespec ts = current_kernel_time();
    STM_LOCK;
        struct entt_ioctl_ait_data *dt = (struct entt_ioctl_ait_data *) sendq_peek(mcn);
    STM_UNLOCK;
    // FIXME: access not under lock:
    int pending = sendq_count(mcn);
    // int nfree = sendq_space(mcn);
    // int avail = recvq_space(mcn);
    int delivered = recvq_count(mcn);
    if (dt) {
        dt->num_messages = delivered;
        dt->num_queued = pending;
        STM_TDEBUG("sendq_peek - pending %d recvq delivered %d", pending, delivered);
    }
    else {
        STM_TDEBUG("sendq_peek - empty, recvq delivered %d", delivered);
    }
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
    STM_LOCK;
        struct entt_ioctl_ait_data *dt = recvq_pop(mcn);
    STM_UNLOCK;
    int pending = sendq_count(mcn);
    // int nfree = sendq_space(mcn);
    // int avail = recvq_space(mcn);
    int delivered = recvq_count(mcn);
    if (dt) {
        // FIXME: access not under lock:
        dt->num_messages = delivered;
        dt->num_queued = pending;
        STM_TDEBUG("recvq_pop - delivered %d sendq pending %d", delivered, pending);
    }
    else {
        // FIXME: should allocate and return an 'empty' dt w/counts
        STM_TDEBUG("recvq_pop - empty, sendq pending %d", pending);
    }
    return dt;
}

// number of pending xmits
uint16_t entl_num_queued(entl_state_machine_t *mcn) {
    // struct timespec ts = current_kernel_time();
    STM_LOCK;
    uint16_t pending = sendq_count(mcn);
    // int nfree = sendq_space(mcn);
    // int avail = recvq_space(mcn);
    // int delivered = recvq_count(mcn);
    STM_UNLOCK;
    // don't log because only used for info, not logic
    // STM_TDEBUG("sendq_count %d", pending);
    return pending;
}

// eof
