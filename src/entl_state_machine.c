#include <linux/types.h>
#include <linux/string.h>
#include <linux/module.h>
#include <linux/slab.h>

#include "entl_state_machine.h"
#include "entt_queue.h"
#include "entl_stm_if.h"
#include "entl_user_api.h"

// FIXME: duplicate defn
#define ENTL_DEBUG(fmt, args...) printk(KERN_ALERT "ENTL:" fmt, ## args)
#define STM_TDEBUG(fmt, args...) ENTL_DEBUG(" %ld %s " fmt "\n", ts.tv_sec, mcn->name, ## args)

#define STM_LOCK unsigned long flags; spin_lock_irqsave(&mcn->state_lock, flags)
#define STM_UNLOCK spin_unlock_irqrestore(&mcn->state_lock, flags)
#define OOPS_STM_UNLOCK spin_unlock(&mcn->state_lock)

static inline int cmp_addr(uint16_t l_high, uint32_t l_low, uint16_t r_high, uint32_t r_low) {
    if (l_high > r_high) return 1;
    if (l_high < r_high) return -1;
    return l_low - r_low;
}

void entl_set_my_adder(entl_state_machine_t *mcn, uint16_t u_addr, uint32_t l_addr) {
    struct timespec ts = current_kernel_time();
    STM_TDEBUG("set macaddr %04x %08x", u_addr, l_addr);
    STM_LOCK;
        mcn->my_u_addr = u_addr;
        mcn->my_l_addr = l_addr;
        mcn->my_addr_valid = 1;
        mcn->hello_addr_valid = 0;
    STM_UNLOCK;
}

// unused ??
uint32_t get_entl_state(entl_state_machine_t *mcn) {
    STM_LOCK;
        uint16_t ret_state = (mcn->error_state.error_count) ? ENTL_STATE_ERROR : get_atomic_state(mcn);
    STM_UNLOCK; // OOPS_STM_UNLOCK;
    return ret_state;
}

int entl_received(entl_state_machine_t *mcn, uint16_t u_saddr, uint32_t l_saddr, uint16_t u_daddr, uint32_t l_daddr) {
    struct timespec ts = current_kernel_time();

    if (get_entl_msg(u_daddr) == ENTL_MESSAGE_NOP_U) return ENTL_ACTION_NOP;

    if (mcn->my_addr_valid == 0) {
        STM_TDEBUG("invalid, macaddr %04x %08x", mcn->my_u_addr, mcn->my_l_addr);
        return ENTL_ACTION_NOP;
    }

    if (mcn->error_state.error_count) {
        STM_TDEBUG("message 0x%04x, error count %d", u_daddr, mcn->error_state.error_count);
        return ENTL_ACTION_SIG_ERR;
    }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        switch (get_atomic_state(mcn)) {
        case ENTL_STATE_IDLE: {
            STM_TDEBUG("message 0x%04x, Idle state", u_daddr);
        }
        ret_action = ENTL_ACTION_NOP;
        break;

        case ENTL_STATE_HELLO: {
            if (get_entl_msg(u_daddr) == ENTL_MESSAGE_HELLO_U) {
                mcn->hello_u_addr = u_saddr;
                mcn->hello_l_addr = l_saddr;
                mcn->hello_addr_valid = 1;

                int ordering = cmp_addr(mcn->my_u_addr, mcn->my_l_addr, u_saddr, l_saddr);
                if (ordering > 0) {
                // if ((mcn->my_u_addr > u_saddr) ||  ((mcn->my_u_addr == u_saddr) && (mcn->my_l_addr > l_saddr))) { // }
                    set_i_sent(mcn, 0);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_WAIT);
                    set_update_time(mcn, ts);
                    clear_intervals(mcn);
                    mcn->state_count = 0;
                    STM_TDEBUG("Hello message %d, hello state and win -> Wait state", u_saddr);
                    ret_action = ENTL_ACTION_SEND;
                }
                else if (ordering == 0) {
                // else if ((mcn->my_u_addr == u_saddr) && (mcn->my_l_addr == l_saddr)) { // }
                    // say error as Alan's 1990s problem again
                    STM_TDEBUG("Fatal Error - hello, SAME ADDRESS");
                    set_error(mcn, ENTL_ERROR_SAME_ADDRESS);
                    set_atomic_state(mcn, ENTL_STATE_IDLE);
                    set_update_time(mcn, ts);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    STM_TDEBUG("Hello message %d, wait state but not win", u_saddr);
                    ret_action = ENTL_ACTION_NOP;
                }
            }
            else if (get_entl_msg(u_daddr) == ENTL_MESSAGE_EVENT_U) {
                // Hello state got event
                if (l_daddr == 0) {
                    set_i_know(mcn, l_daddr);
                    set_send_next(mcn, l_daddr + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    calc_intervals(mcn);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("message %d, Hello -> Send", l_daddr);
                    ret_action = ENTL_ACTION_SEND;
                }
                else {
                    STM_TDEBUG("Out of sequence: message %d, Hello", l_daddr);
                    ret_action = ENTL_ACTION_NOP;
                }
            }
            else {
                STM_TDEBUG("non-hello message 0x%04x, hello state", u_daddr);
                ret_action = ENTL_ACTION_NOP;
            }
        }
        break;

        case ENTL_STATE_WAIT: {
            if (get_entl_msg(u_daddr) == ENTL_MESSAGE_HELLO_U) {
                mcn->state_count++;
                if (mcn->state_count > ENTL_COUNT_MAX) {
                    STM_TDEBUG("Hello message %d, overflow %d, Wait -> Hello", u_saddr, mcn->state_count);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                }
                ret_action = ENTL_ACTION_NOP;
            }
            else if (get_entl_msg(u_daddr) == ENTL_MESSAGE_EVENT_U) {
                if (l_daddr == get_i_sent(mcn) + 1) {
                    set_i_know(mcn, l_daddr);
                    set_send_next(mcn, l_daddr + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    set_update_time(mcn, ts);
                    clear_intervals(mcn);
                    STM_TDEBUG("message %d, Wait -> Send", l_daddr);
                    ret_action = ENTL_ACTION_SEND;
                }
                else {
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    clear_intervals(mcn);
                    STM_TDEBUG("Wrong message %d, Wait -> Hello", l_daddr);
                    ret_action = ENTL_ACTION_NOP;
                }
            }
            else {
                // Received non hello message on Wait state
                STM_TDEBUG("wrong message 0x%04x, Wait -> Hello", u_daddr);
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                ret_action = 0;
            }
        }
        break;

        case ENTL_STATE_SEND: {
            if (get_entl_msg(u_daddr) == ENTL_MESSAGE_EVENT_U
            ||  get_entl_msg(u_daddr) == ENTL_MESSAGE_ACK_U) {
                if (l_daddr == get_i_know(mcn)) {
                    STM_TDEBUG("Same message %d, Send", l_daddr);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("Out of Sequence message %d, Send -> Hello", l_daddr);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                STM_TDEBUG("wrong message 0x%04x, Send -> Hello", u_daddr);
                ret_action = ENTL_ACTION_ERROR;
            }
        }
        break;

        case ENTL_STATE_RECEIVE: {
            if (get_entl_msg(u_daddr) == ENTL_MESSAGE_EVENT_U) {
                if (get_i_know(mcn) + 2 == l_daddr) {
                    set_i_know(mcn, l_daddr);
                    set_send_next(mcn, l_daddr + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    ret_action = ENTL_ACTION_SEND;
                    if (!mcn->send_ATI_queue.count) { // AIT has priority
                        ret_action |= ENTL_ACTION_SEND_DAT; // data send as optional
                    }
                    set_update_time(mcn, ts);
                }
                else if (get_i_know(mcn) == l_daddr) {
                    STM_TDEBUG("same message %d, Receive", l_daddr);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("Out of Sequence message %d, Receive -> Hello", l_daddr);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else if (get_entl_msg(u_daddr) == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) + 2 == l_daddr) {
                    set_i_know(mcn, l_daddr);
                    set_send_next(mcn, l_daddr + 1);
                    set_atomic_state(mcn, ENTL_STATE_AH);
                    ret_action = ENTL_ACTION_PROC_AIT;
                    if (!ENTT_queue_full(&mcn->receive_ATI_queue)) {
                        ret_action |= ENTL_ACTION_SEND;
                    }
                    else {
                        STM_TDEBUG("message %d, queue full -> Ah", l_daddr);
                    }
                    set_update_time(mcn, ts);
                    STM_TDEBUG("message %d, Receive -> Ah", l_daddr);
                }
                else if (get_i_know(mcn) == l_daddr) {
                    STM_TDEBUG("same message %d, Receive", l_daddr);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("Out of Sequence message %d, Receive -> Hello", l_daddr);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                STM_TDEBUG("Wrong message 0x%04x, Receive -> Hello", u_daddr);
                ret_action = ENTL_ACTION_ERROR;
            }
        }
        break;

        // AIT message sent, waiting for ack
        case ENTL_STATE_AM: {
            if (get_entl_msg(u_daddr) == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) + 2 == l_daddr) {
                    set_i_know(mcn, l_daddr);
                    set_send_next(mcn, l_daddr + 1);
                    set_atomic_state(mcn, ENTL_STATE_BM);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("ETL Ack message %d, Am -> Bm", l_daddr);
                    ret_action = ENTL_ACTION_SEND;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("Out of Sequence message %d, Am -> Hello", l_daddr);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else if (get_entl_msg(u_daddr) == ENTL_MESSAGE_EVENT_U) {
                if (get_i_know(mcn) == l_daddr) {
                    STM_TDEBUG("same ETL event message %d, Am", l_daddr);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("Wrong message 0x%04x, Am -> Hello", u_daddr);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                STM_TDEBUG("Wrong message 0x%04x, Am -> Hello", u_daddr);
                ret_action = ENTL_ACTION_ERROR;
            }
        }
        break;

        // AIT sent, Ack received, sending Ack
        case ENTL_STATE_BM: {
            if (get_entl_msg(u_daddr) == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) == l_daddr) {
                    STM_TDEBUG("same ETL Ack message %d, Bm", l_daddr);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("Wrong message 0x%04x, Bm -> Hello", u_daddr);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                STM_TDEBUG("Wrong message 0x%04x, Bm -> Hello", u_daddr);
                ret_action = ENTL_ACTION_ERROR;
            }
        }
        break;

        // AIT message received, sending Ack
        case ENTL_STATE_AH: {
            if (get_entl_msg(u_daddr) == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) == l_daddr) {
                    STM_TDEBUG("Same ENTL message %d, Ah", l_daddr);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("Out of Sequence message %d, Ah -> Hello", l_daddr);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                STM_TDEBUG("wrong message 0x%04x, Send -> Hello", u_daddr);
                ret_action = ENTL_ACTION_ERROR;
            }
        }
        break;

        // got AIT, Ack sent, waiting for ack
        case ENTL_STATE_BH: {
            if (get_entl_msg(u_daddr) == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) + 2 == l_daddr) {
                    set_i_know(mcn, l_daddr);
                    set_send_next(mcn, l_daddr + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("ETL Ack message %d, Bh -> Send", l_daddr);
                    ENTT_queue_back_push(&mcn->receive_ATI_queue, mcn->receive_buffer);
                    mcn->receive_buffer = NULL;
                    ret_action = ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("Out of Sequence message %d, Am -> Hello", l_daddr);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else if (get_entl_msg(u_daddr) == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) == l_daddr) {
                    STM_TDEBUG("Same ENTL message %d, Bh", l_daddr);
                    ret_action = ENTL_ACTION_NOP;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    STM_TDEBUG("Out of Sequence message %d, Bh -> Hello", l_daddr);
                    ret_action = ENTL_ACTION_ERROR;
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                STM_TDEBUG("Wrong message 0x%04x, Am -> Hello", u_daddr);
                ret_action = ENTL_ACTION_ERROR;
            }
        }
        break;

        default: {
            STM_TDEBUG("wrong state %d", get_atomic_state(mcn));
            set_error(mcn, ENTL_ERROR_UNKOWN_STATE);
            set_i_know(mcn, 0);
            set_send_next(mcn, 0);
            set_i_sent(mcn, 0);
            set_atomic_state(mcn, ENTL_STATE_IDLE);
            set_update_time(mcn, ts);
        }
        ret_action = ENTL_ACTION_NOP;
        break;
    }
    STM_UNLOCK;
    return ret_action;
}

int entl_get_hello(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr) {
    struct timespec ts = current_kernel_time();

    if (mcn->error_state.error_count) {
        STM_TDEBUG("entl_get_hello, error count %d", mcn->error_state.error_count);
        return ENTL_ACTION_NOP;
    }

#define hello_next(hi, lo, action) { *u_addr = hi; *l_addr = lo; ret_action = action; }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        switch (get_atomic_state(mcn)) {
        case ENTL_STATE_HELLO:
            hello_next(ENTL_MESSAGE_HELLO_U, ENTL_MESSAGE_HELLO_L, ENTL_ACTION_SEND);
        break;

        case ENTL_STATE_WAIT:
            hello_next(ENTL_MESSAGE_EVENT_U, 0, ENTL_ACTION_SEND);
        break;

        case ENTL_STATE_RECEIVE:
            hello_next(ENTL_MESSAGE_EVENT_U, get_i_sent(mcn), ENTL_ACTION_SEND);
            STM_TDEBUG("repeat EVENT, Receive");
        break;

        case ENTL_STATE_AM:
            hello_next(ENTL_MESSAGE_AIT_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SEND_AIT);
            STM_TDEBUG("repeat AIT, Am");
        break;

        case ENTL_STATE_BH:
            if (!ENTT_queue_full(&mcn->receive_ATI_queue)) {
                hello_next(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND);
                STM_TDEBUG("repeat ACK, Bh");
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

int entl_next_send(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr) {
    struct timespec ts = current_kernel_time();

    if (mcn->error_state.error_count) {
        *u_addr = ENTL_MESSAGE_NOP_U; *l_addr = 0;
        STM_TDEBUG("entl_next_send, error count %d", mcn->error_state.error_count);
        return ENTL_ACTION_NOP;
    }

#define xxx(hi, lo, action) { *u_addr = hi; *l_addr = lo; ret_action = action; }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        switch (get_atomic_state(mcn)) {
        case ENTL_STATE_IDLE:
            xxx(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            STM_TDEBUG("Message requested, Idle");
        break;

        case ENTL_STATE_HELLO:
            xxx(ENTL_MESSAGE_HELLO_U, ENTL_MESSAGE_HELLO_L, ENTL_ACTION_SEND);
        break;

        case ENTL_STATE_WAIT:
            xxx(ENTL_MESSAGE_EVENT_U, 0, ENTL_ACTION_NOP);
        break;

        case ENTL_STATE_SEND: {
            uint32_t event_i_know = get_i_know(mcn); // last received event number
            uint32_t event_i_sent = get_i_sent(mcn);
            zebra(mcn);
            advance_send_next(mcn);
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            // Avoiding to send AIT on the very first loop where other side will be in Hello state
            if (event_i_know && event_i_sent && mcn->send_ATI_queue.count) {
                set_atomic_state(mcn, ENTL_STATE_AM);
                STM_TDEBUG("ETL AIT message %d, Send -> Am", *l_addr);
                xxx(ENTL_MESSAGE_AIT_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SEND_AIT);
            }
            else {
                set_atomic_state(mcn, ENTL_STATE_RECEIVE);
                xxx(ENTL_MESSAGE_EVENT_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SEND_DAT); // data send as optional
            }
        }
        break;

        case ENTL_STATE_RECEIVE:
            xxx(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;

        // AIT
        case ENTL_STATE_AM:
            xxx(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;

        case ENTL_STATE_BM: {
            struct entt_ioctl_ait_data *ait_data;
            zebra(mcn);
            advance_send_next(mcn);
            xxx(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT);
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            // drop the message on the top
            ait_data = ENTT_queue_front_pop(&mcn->send_ATI_queue);
            if (ait_data) {
                kfree(ait_data);
            }
            STM_TDEBUG("ETL AIT ACK message %d, BM -> Receive", *l_addr);
        }
        break;

        case ENTL_STATE_AH: {
            if (ENTT_queue_full(&mcn->receive_ATI_queue)) {
                xxx(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            }
            else {
                zebra(mcn);
                advance_send_next(mcn);
                xxx(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND);
                calc_intervals(mcn);
                set_update_time(mcn, ts);
                set_atomic_state(mcn, ENTL_STATE_BH);
                STM_TDEBUG("ETL AIT ACK message %d, Ah -> Bh", *l_addr);
            }
        }
        break;

        case ENTL_STATE_BH:
            xxx(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;

        default:
            xxx(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;
        }
    STM_UNLOCK;
    return ret_action;
}

// For TX, it can't send AIT, so just keep ENTL state on Send state
int entl_next_send_tx(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr) {
    struct timespec ts = current_kernel_time();

    if (mcn->error_state.error_count) {
        *u_addr = ENTL_MESSAGE_NOP_U; *l_addr = 0;
        STM_TDEBUG("entl_next_send_tx, error count %d", mcn->error_state.error_count);
        return ENTL_ACTION_NOP;
    }

#define yyy(hi, lo, action) { *u_addr = hi; *l_addr = lo; ret_action = action; }

    int ret_action = ENTL_ACTION_NOP;
    STM_LOCK;
        switch (get_atomic_state(mcn)) {
        case ENTL_STATE_IDLE:
            yyy(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            STM_TDEBUG("Message requested on Idle state");
        break;

        case ENTL_STATE_HELLO:
            yyy(ENTL_MESSAGE_HELLO_U, ENTL_MESSAGE_HELLO_L, ENTL_ACTION_SEND);
        break;

        case ENTL_STATE_WAIT:
            yyy(ENTL_MESSAGE_EVENT_U, 0, ENTL_ACTION_NOP);
        break;

        case ENTL_STATE_SEND: {
            zebra(mcn);
            advance_send_next(mcn);
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            yyy(ENTL_MESSAGE_EVENT_U, get_i_sent(mcn), ENTL_ACTION_SEND);
            // For TX, it can't send AIT, so just keep ENTL state on Send state
        }
        break;

        case ENTL_STATE_RECEIVE:
            yyy(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;

        // AIT
        case ENTL_STATE_AM:
            yyy(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;

        case ENTL_STATE_BM: {
            zebra(mcn);
            advance_send_next(mcn);
            yyy(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT);
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            // drop the message on the top
            ENTT_queue_front_pop(&mcn->send_ATI_queue);
            STM_TDEBUG("ETL AIT ACK message %d, BM -> Receive", *l_addr);
        }
        break;

        case ENTL_STATE_AH: {
            if (ENTT_queue_full(&mcn->receive_ATI_queue)) {
                yyy(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
            }
            else {
                zebra(mcn);
                advance_send_next(mcn);
                yyy(ENTL_MESSAGE_ACK_U, get_i_sent(mcn), ENTL_ACTION_SEND);
                calc_intervals(mcn);
                set_update_time(mcn, ts);
                set_atomic_state(mcn, ENTL_STATE_BH);
                STM_TDEBUG("ETL AIT ACK message %d, Ah -> Bh", *l_addr);
            }
        }
        break;

        case ENTL_STATE_BH:
            yyy(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
        break;

        default:
            yyy(ENTL_MESSAGE_NOP_U, 0, ENTL_ACTION_NOP);
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
            set_atomic_state(mcn, ENTL_STATE_HELLO);
            set_update_time(mcn, ts);
            clear_error(mcn);
            // when following 3 members are all zero, it means fresh out of Hello handshake
            set_i_sent(mcn, 0);
            set_i_know(mcn, 0);
            set_send_next(mcn, 0);
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
            STM_TDEBUG("Link Up, error count %d, ignored", mcn->error_state.error_count);
        }
        else {
            STM_TDEBUG("Link Up");
            set_atomic_state(mcn, ENTL_STATE_HELLO);
            set_update_time(mcn, ts);
            clear_error(mcn);
            set_i_sent(mcn, 0);
            set_i_know(mcn, 0);
            set_send_next(mcn, 0);
            clear_intervals(mcn);
        }
    STM_UNLOCK;
}

// AIT handling functions
// Request to send the AIT message, return 0 if OK, -1 if queue full
int entl_send_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data) {
    STM_LOCK;
        int ret = ENTT_queue_back_push(&mcn->send_ATI_queue, (void *) data);
    STM_UNLOCK;
    return ret;
}

// Read the next AIT message to send
struct entt_ioctl_ait_data *entl_next_AIT_message(entl_state_machine_t *mcn) {
    STM_LOCK;
        struct entt_ioctl_ait_data *dt = (struct entt_ioctl_ait_data *) ENTT_queue_front(&mcn->send_ATI_queue);
    STM_UNLOCK;
    return dt;
}

// the new AIT message received
void entl_new_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data) {
    STM_LOCK;
        mcn->receive_buffer = data;
    STM_UNLOCK;
}

// Read the AIT message, return NULL if queue empty
struct entt_ioctl_ait_data *entl_read_AIT_message(entl_state_machine_t *mcn) {
    STM_LOCK;
        struct entt_ioctl_ait_data *dt = ENTT_queue_front_pop(&mcn->receive_ATI_queue);
        if (dt) {
            dt->num_messages = mcn->receive_ATI_queue.count; // return how many left
            dt->num_queued = mcn->send_ATI_queue.count;
        }
    STM_UNLOCK;
    return dt;
}

uint16_t entl_num_queued(entl_state_machine_t *mcn) {
    STM_LOCK;
        uint16_t count = mcn->send_ATI_queue.count;
    STM_UNLOCK;
    return count;
}
