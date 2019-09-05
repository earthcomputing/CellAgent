#include <linux/types.h>
#include <linux/string.h>
#include <linux/module.h>
#include <linux/slab.h>

#include "entl_state_machine.h"
#include "entt_queue.h"
#include "entl_stm_if.h"
#include "entl_user_api.h"

// FIXME: duplicate defn
// newline should be unnecessary here - https://lwn.net/Articles/732420/
#define ENTL_DEBUG(fmt, args...) printk(KERN_ALERT "ENTL: " fmt "\n", ## args)
#define STM_TDEBUG(fmt, args...) ENTL_DEBUG("%ld %s " fmt "\n", ts.tv_sec, mcn->name, ## args)
#define STM_TDEBUG_ERROR(mcn, fmt, args...) STM_TDEBUG("error pending: flag %d count %d " fmt, mcn->error_state.error_flag, mcn->error_state.error_count, ## args)

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
    STM_TDEBUG("set macaddr %04x %08x", mac_hi, mac_lo);
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
        uint16_t ret_state = (mcn->error_state.error_count) ? ENTL_STATE_ERROR : get_atomic_state(mcn);
    STM_UNLOCK; // OOPS_STM_UNLOCK;
    return ret_state;
}

static inline char *msg_nick(int emsg_type) { return (emsg_type == ENTL_MESSAGE_EVENT_U) ? "EVENT" : (emsg_type == ENTL_MESSAGE_ACK_U) ? "ACK" : "??"; }

int entl_received(entl_state_machine_t *mcn, uint16_t from_hi, uint32_t from_lo, uint16_t emsg_raw, uint32_t seqno) {
    struct timespec ts = current_kernel_time();
    uint16_t emsg_type = get_entl_msg(emsg_raw);

    if (emsg_type == ENTL_MESSAGE_NOP_U) return ENTL_ACTION_NOP;

    if (mcn->mac_valid == 0) {
        STM_TDEBUG("invalid, macaddr %04x %08x", mcn->mac_hi, mcn->mac_lo);
        return ENTL_ACTION_NOP;
    }

    if (mcn->error_state.error_count) {
        STM_TDEBUG_ERROR(mcn, "message 0x%04x", emsg_raw);
        return ENTL_ACTION_SIG_ERR;
    }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        switch (get_atomic_state(mcn)) {
        case ENTL_STATE_IDLE: {
            STM_TDEBUG("message 0x%04x, IDLE", emsg_raw);
        }
        ret_action = ENTL_ACTION_NOP;
        break;

        case ENTL_STATE_HELLO: {
            if (emsg_type == ENTL_MESSAGE_HELLO_U) {
                mcn->hello_hi = from_hi;
                mcn->hello_lo = from_lo;
                mcn->hello_valid = 1;

                int ordering = cmp_addr(mcn->mac_hi, mcn->mac_lo, from_hi, from_lo);
                if (ordering > 0) {
                // if ((mcn->mac_hi > from_hi) ||  ((mcn->mac_hi == from_hi) && (mcn->mac_lo > from_lo))) { // }
                    unicorn(mcn, ENTL_STATE_WAIT);
                    set_update_time(mcn, ts);
                    clear_intervals(mcn);
                    mcn->state_count = 0;
                    STM_TDEBUG("neighbor %04x %08x, HELLO (master) -> WAIT", from_hi, from_lo);
                    ret_action = ENTL_ACTION_SEND;
                }
                else if (ordering == 0) {
                // else if ((mcn->mac_hi == from_hi) && (mcn->mac_lo == from_lo)) { // }
                    // say error as Alan's 1990s problem again
                    STM_TDEBUG("neighbor %04x %08x, Fatal Error - HELLO, SAME ADDRESS", from_hi, from_lo);
                    set_error(mcn, ENTL_ERROR_SAME_ADDRESS);
                    set_atomic_state(mcn, ENTL_STATE_IDLE);
                    set_update_time(mcn, ts);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("neighbor %04x %08x, HELLO (slave)", from_hi, from_lo);
                    ret_action = ENTL_ACTION_NOP;
                }
            }
            else if (emsg_type == ENTL_MESSAGE_EVENT_U) {
                // Hello state got event
                if (seqno == 0) {
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    calc_intervals(mcn);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("EVENT: seqno %d, HELLO (slave) -> SEND", seqno);
                    ret_action = ENTL_ACTION_SEND;
                }
                else {
                    STM_TDEBUG("EVENT: Out of Sequence: seqno %d, HELLO", seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
            }
            else {
                STM_TDEBUG("message 0x%04x, HELLO", emsg_raw);
                ret_action = ENTL_ACTION_NOP;
            }
        }
        break;

        case ENTL_STATE_WAIT: {
            if (emsg_type == ENTL_MESSAGE_HELLO_U) {
                mcn->state_count++; // private to this logic
                if (mcn->state_count > ENTL_COUNT_MAX) {
                    STM_TDEBUG("overflow %d, WAIT -> HELLO", mcn->state_count);
                    unicorn(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                }
                ret_action = ENTL_ACTION_NOP;
            }
            else if (emsg_type == ENTL_MESSAGE_EVENT_U) {
                // hmm, this should be exactly 1, not sent+1
                if (seqno == get_i_sent(mcn) + 1) {
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    set_update_time(mcn, ts);
                    clear_intervals(mcn);
                    STM_TDEBUG("EVENT: seqno %d, WAIT (master) -> SEND", seqno);
                    ret_action = ENTL_ACTION_SEND;
                }
                else {
                    unicorn(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    clear_intervals(mcn);
                    STM_TDEBUG("EVENT: Wrong seqno %d, WAIT -> HELLO", seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
            }
            else {
                // Received non hello message on Wait state
                STM_TDEBUG("wrong message 0x%04x, WAIT -> HELLO", emsg_raw);
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                unicorn(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                ret_action = 0; // ENTL_ACTION_NOP
            }
        }
        break;

        case ENTL_STATE_SEND: {
            if (emsg_type == ENTL_MESSAGE_EVENT_U
            ||  emsg_type == ENTL_MESSAGE_ACK_U) {
                if (seqno == get_i_know(mcn)) {
                    STM_TDEBUG("%s same seqno %d, SEND", msg_nick(emsg_type), seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    unicorn(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("%s Out of Sequence: seqno %d, SEND -> HELLO", msg_nick(emsg_type), seqno);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                unicorn(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                STM_TDEBUG("wrong message 0x%04x, SEND -> HELLO", emsg_raw);
                ret_action = ENTL_ACTION_ERROR;
            }
        }
        break;

        case ENTL_STATE_RECEIVE: {
            if (emsg_type == ENTL_MESSAGE_EVENT_U) {
                if (get_i_know(mcn) + 2 == seqno) {
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    ret_action = ENTL_ACTION_SEND;
// send queue non-empty
                    if (!sendq_count(mcn)) { // AIT has priority
                        ret_action |= ENTL_ACTION_SEND_DAT; // data send as optional
                    }
                    set_update_time(mcn, ts);
                }
                else if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("EVENT: same seqno %d, RECEIVE", seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    unicorn(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("EVENT: Out of Sequence: seqno %d, RECEIVE -> HELLO", seqno);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else if (emsg_type == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) + 2 == seqno) {
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_AH);
                    ret_action = ENTL_ACTION_PROC_AIT;
// recv queue space avail
                    if (!recvq_full(mcn)) {
                        ret_action |= ENTL_ACTION_SEND;
                    }
                    else {
                        STM_TDEBUG("AIT: queue full seqno %d, RECEIVE -> AH", seqno);
                    }
                    set_update_time(mcn, ts);
                    STM_TDEBUG("AIT: seqno %d, RECEIVE -> AH", seqno);
                }
                else if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("AIT: same seqno %d, RECEIVE", seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    unicorn(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("AIT: Out of Sequence: seqno %d, RECEIVE -> HELLO", seqno);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                unicorn(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                STM_TDEBUG("Wrong message 0x%04x, RECEIVE -> HELLO", emsg_raw);
                ret_action = ENTL_ACTION_ERROR;
            }
        }
        break;

        // AIT message sent, waiting for ack
        case ENTL_STATE_AM: {
            if (emsg_type == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) + 2 == seqno) {
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_BM);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("ACK: seqno %d, AM -> BM", seqno);
                    ret_action = ENTL_ACTION_SEND;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    unicorn(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("ACK: Out of Sequence: seqno %d, AM -> HELLO", seqno);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else if (emsg_type == ENTL_MESSAGE_EVENT_U) {
                if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("EVENT: same ETL event seqno %d, AM", seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    unicorn(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("EVENT: Wrong message 0x%04x, AM -> HELLO", emsg_raw);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                unicorn(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                STM_TDEBUG("Wrong message 0x%04x, AM -> HELLO", emsg_raw);
                ret_action = ENTL_ACTION_ERROR;
            }
        }
        break;

        // AIT sent, Ack received, sending Ack
        case ENTL_STATE_BM: {
            if (emsg_type == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("ACK: same seqno %d, BM", seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    unicorn(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("ACK: Wrong message 0x%04x, BM -> HELLO", emsg_raw);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                unicorn(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                STM_TDEBUG("Wrong message 0x%04x, BM -> HELLO", emsg_raw);
                ret_action = ENTL_ACTION_ERROR;
            }
        }
        break;

        // AIT message received, sending Ack
        case ENTL_STATE_AH: {
            if (emsg_type == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("AIT: same ENTL seqno %d, AH", seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    unicorn(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("AIT: Out of Sequence: seqno %d, AH -> HELLO", seqno);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                unicorn(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                STM_TDEBUG("wrong message 0x%04x, AH -> HELLO", emsg_raw);
                ret_action = ENTL_ACTION_ERROR;
            }
        }
        break;

        // got AIT, Ack sent, waiting for ack
        case ENTL_STATE_BH: {
            if (emsg_type == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) + 2 == seqno) {
                    set_i_know(mcn, seqno); set_send_next(mcn, seqno + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("ACK: seqno %d, BH -> SEND", seqno);
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
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    unicorn(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("ACK: Out of Sequence seqno %d, BH -> HELLO", seqno);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else if (emsg_type == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) == seqno) {
                    STM_TDEBUG("AIT: same ENTL seqno %d, BH", seqno);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    unicorn(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("AIT: Out of Sequence: seqno %d, BH -> HELLO", seqno);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                unicorn(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                STM_TDEBUG("Wrong message 0x%04x, BH -> HELLO", emsg_raw);
                ret_action = ENTL_ACTION_ERROR;
            }
        }
        break;

        default: {
            STM_TDEBUG("wrong state %d", get_atomic_state(mcn));
            set_error(mcn, ENTL_ERROR_UNKOWN_STATE);
            unicorn(mcn, ENTL_STATE_IDLE);
            set_update_time(mcn, ts);
        }
        ret_action = ENTL_ACTION_NOP;
        break;
    }
    STM_UNLOCK;
    return ret_action;
}

int entl_get_hello(entl_state_machine_t *mcn, uint16_t *emsg_raw, uint32_t *seqno) {
    struct timespec ts = current_kernel_time();

    if (mcn->error_state.error_count) {
        STM_TDEBUG_ERROR(mcn, "entl_get_hello");
        return ENTL_ACTION_NOP;
    }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        switch (get_atomic_state(mcn)) {
        case ENTL_STATE_HELLO:
            respond_with(ENTL_MESSAGE_HELLO_U, ENTL_MESSAGE_HELLO_L, ENTL_ACTION_SEND);
        break;

        case ENTL_STATE_WAIT:
            respond_with(ENTL_MESSAGE_EVENT_U, 0, ENTL_ACTION_SEND);
        break;

        case ENTL_STATE_RECEIVE:
            respond_with(ENTL_MESSAGE_EVENT_U, get_i_sent(mcn), ENTL_ACTION_SEND);
            STM_TDEBUG("EVENT(out): RECEIVE");
        break;

        case ENTL_STATE_AM:
            respond_with(ENTL_MESSAGE_AIT_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SEND_AIT);
            STM_TDEBUG("AIT(out): AM");
        break;

        case ENTL_STATE_BH:
// recv queue space avail
            if (!recvq_full(mcn)) {
                respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND);
                STM_TDEBUG("ACK(out): BH");
            }
            else {
                ret_action = ENTL_ACTION_NOP;
            }
        break;

        default:
            ret_action = ENTL_ACTION_NOP;
        break;
        }
    STM_UNLOCK;
    return ret_action;
}

int entl_next_send(entl_state_machine_t *mcn, uint16_t *emsg_raw, uint32_t *seqno) {
    struct timespec ts = current_kernel_time();

    if (mcn->error_state.error_count) {
        int ret_action;
        respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        STM_TDEBUG_ERROR(mcn, "entl_next_send");
        return ret_action;
    }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        switch (get_atomic_state(mcn)) {
        case ENTL_STATE_IDLE:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            STM_TDEBUG("NOP(out): IDLE");
        break;

        case ENTL_STATE_HELLO:
            respond_with(ENTL_MESSAGE_HELLO_U, ENTL_MESSAGE_HELLO_L, ENTL_ACTION_SEND);
        break;

        case ENTL_STATE_WAIT:
            respond_with(ENTL_MESSAGE_EVENT_U, 0, ENTL_ACTION_NOP);
        break;

        case ENTL_STATE_SEND: {
            uint32_t event_i_know = get_i_know(mcn); // last received event number
            uint32_t event_i_sent = get_i_sent(mcn);
            zebra(mcn); advance_send_next(mcn);
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            // Avoiding to send AIT on the very first loop where other side will be in Hello state
// send queue non-empty
            if (event_i_know && event_i_sent && sendq_count(mcn)) {
                set_atomic_state(mcn, ENTL_STATE_AM);
                respond_with(ENTL_MESSAGE_AIT_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SEND_AIT);
                STM_TDEBUG("AIT(out): seqno %d, SEND -> AM", *seqno);
            }
            else {
                set_atomic_state(mcn, ENTL_STATE_RECEIVE);
                respond_with(ENTL_MESSAGE_EVENT_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SEND_DAT); // data send as optional
            }
        }
        break;

        case ENTL_STATE_RECEIVE:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;

        // AIT
        case ENTL_STATE_AM:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;

        case ENTL_STATE_BM: {
            zebra(mcn); advance_send_next(mcn);
            respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT);
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            STM_TDEBUG("ACK(out): seqno %d, BM -> RECEIVE", *seqno);
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
                set_atomic_state(mcn, ENTL_STATE_BH);
// space avail
                STM_TDEBUG("ACK(out): seqno %d, AH -> BH", *seqno);
                calc_intervals(mcn);
                set_update_time(mcn, ts);
            }
            else {
                respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            }
        }
        break;

        case ENTL_STATE_BH:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;

        default:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;
        }
    STM_UNLOCK;
    return ret_action;
}

// For TX, it can't send AIT, so just keep ENTL state on Send state
int entl_next_send_tx(entl_state_machine_t *mcn, uint16_t *emsg_raw, uint32_t *seqno) {
    struct timespec ts = current_kernel_time();

    // might be offline(no carrier), or be newly online after offline ??
    if (mcn->error_state.error_count) {
        int ret_action;
        respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        STM_TDEBUG_ERROR(mcn, "entl_next_send_tx");
        return ret_action;
    }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        switch (get_atomic_state(mcn)) {
        case ENTL_STATE_IDLE:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            STM_TDEBUG("NOP(out): IDLE");
        break;

        case ENTL_STATE_HELLO:
            respond_with(ENTL_MESSAGE_HELLO_U, ENTL_MESSAGE_HELLO_L, ENTL_ACTION_SEND);
        break;

        case ENTL_STATE_WAIT:
            respond_with(ENTL_MESSAGE_EVENT_U, 0, ENTL_ACTION_NOP);
        break;

        case ENTL_STATE_SEND: {
            zebra(mcn); advance_send_next(mcn);
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            respond_with(ENTL_MESSAGE_EVENT_U, get_i_sent(mcn), ENTL_ACTION_SEND);
            // For TX, it can't send AIT, so just keep ENTL state on Send state
        }
        break;

        case ENTL_STATE_RECEIVE:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;

        // AIT
        case ENTL_STATE_AM:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;

        case ENTL_STATE_BM: {
            zebra(mcn); advance_send_next(mcn);
            respond_with(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT);
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            STM_TDEBUG("ACK(out): seqno %d, BM -> RECEIVE", *seqno);
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
                set_atomic_state(mcn, ENTL_STATE_BH);
// space avail
                STM_TDEBUG("ACK(out): seqno %d, AH -> BH", *seqno);
                calc_intervals(mcn);
                set_update_time(mcn, ts);
            }
            else {
                respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            }
        }
        break;

        case ENTL_STATE_BH:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;

        default:
            respond_with(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;
        }
    STM_UNLOCK;
    return ret_action;
}

void entl_state_error(entl_state_machine_t *mcn, uint32_t error_flag) {
    struct timespec ts = current_kernel_time();

    if (error_flag == ENTL_ERROR_FLAG_LINKDONW && get_atomic_state(mcn) == ENTL_STATE_IDLE) return;

    STM_LOCK;
        set_error(mcn, error_flag);
        if (error_flag == ENTL_ERROR_FLAG_LINKDONW) {
            set_atomic_state(mcn, ENTL_STATE_IDLE);
        }
        else if (error_flag == ENTL_ERROR_FLAG_SEQUENCE) {
            unicorn(mcn, ENTL_STATE_HELLO);
            set_update_time(mcn, ts);
            clear_error(mcn);
            clear_intervals(mcn);
        }
    STM_UNLOCK;
    STM_TDEBUG("entl_state_error %d, state %d", error_flag, get_atomic_state(mcn));
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
}

void entl_link_up(entl_state_machine_t *mcn) {
    struct timespec ts = current_kernel_time();
    STM_LOCK;
        if (get_atomic_state(mcn) != ENTL_STATE_IDLE) {
            STM_TDEBUG("Link Up, state %d, ignored", get_atomic_state(mcn));
        }
        else if (mcn->error_state.error_count != 0) {
            STM_TDEBUG_ERROR(mcn, "Link Up, error ignored");
        }
        else {
            STM_TDEBUG("Link Up, IDLE -> HELLO");
            unicorn(mcn, ENTL_STATE_HELLO);
            set_update_time(mcn, ts);
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
    return send_space;
}

// peek at next AIT message to xmit
struct entt_ioctl_ait_data *entl_next_AIT_message(entl_state_machine_t *mcn) {
    struct timespec ts = current_kernel_time();
STM_TDEBUG("sendq_peek");
    STM_LOCK;
        struct entt_ioctl_ait_data *dt = (struct entt_ioctl_ait_data *) sendq_peek(mcn);
    STM_UNLOCK;
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
