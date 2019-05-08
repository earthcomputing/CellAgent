#include <linux/types.h>
#include <linux/string.h>
#include <linux/module.h>
#include <linux/slab.h>

#include "entl_state_machine.h"
#include "entt_queue.h"


// FIXME: duplicate defn
#define ENTL_DEBUG(fmt, args...) printk(KERN_ALERT "ENTL:" fmt, ## args)
#define STM_TDEBUG(fmt, args...) ENTL_DEBUG("%s @ %ld sec " fmt "\n", mcn->name, ts.tv_sec, ## args)
#define STM_DEBUG(fmt, args...)  ENTL_DEBUG("%s " fmt "\n", mcn->name, ## args)

#define STM_LOCK unsigned long flags; spin_lock_irqsave(&mcn->state_lock, flags)
#define STM_UNLOCK spin_unlock_irqrestore(&mcn->state_lock, flags)
#define OOPS_STM_UNLOCK spin_unlock(&mcn->state_lock)


void entl_set_my_adder(entl_state_machine_t *mcn, uint16_t u_addr, uint32_t l_addr) {
    STM_DEBUG("set my address %04x %08x", u_addr, l_addr);
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
        uint16_t ret = (mcn->error_state.error_count) ? ENTL_STATE_ERROR : get_atomic_state(mcn);
    OOPS_STM_UNLOCK;
    return ret;
}

int entl_received(entl_state_machine_t *mcn, uint16_t u_saddr, uint32_t l_saddr, uint16_t u_daddr, uint32_t l_daddr) {
    if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_NOP_U) return ENTL_ACTION_NOP;

    if (mcn->my_addr_valid == 0) {
        STM_DEBUG("message received without my address set %04x %08x", mcn->my_u_addr, mcn->my_l_addr);
        return ENTL_ACTION_NOP;
    }

    if (mcn->error_state.error_count) {
        STM_DEBUG("message %04x received on error count set %d", u_daddr, mcn->error_state.error_count);
        return ENTL_ACTION_SIG_ERR;
    }

    struct timespec ts = current_kernel_time();
    int retval = ENTL_ACTION_NOP;
    STM_LOCK;
        switch (get_atomic_state(mcn)) {
        case ENTL_STATE_IDLE: {
            // say something here as receive something on idle state
            STM_TDEBUG("message %x received on Idle state!", u_daddr);
        }
        break;

        case ENTL_STATE_HELLO: {
            if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_HELLO_U) {
                mcn->hello_u_addr = u_saddr;
                mcn->hello_l_addr = l_saddr;
                mcn->hello_addr_valid = 1;
                //STM_TDEBUG("Hello message %d received on hello state", u_saddr);
                if (mcn->my_u_addr > u_saddr || (mcn->my_u_addr == u_saddr && mcn->my_l_addr > l_saddr)) {
                    set_i_sent(mcn, 0);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_WAIT);
                    set_update_time(mcn, ts);
                    clear_intervals(mcn); 
                    retval = ENTL_ACTION_SEND;
                    mcn->state_count = 0;
                    STM_TDEBUG("Hello message %d received on hello state and win -> Wait state", u_saddr);
                }
                else if (mcn->my_u_addr == u_saddr && mcn->my_l_addr == l_saddr) {
                    // say error as Alan's 1990s problem again
                    STM_TDEBUG("Fatal Error!! hello message with SAME MAC ADDRESS received");
                    set_error(mcn, ENTL_ERROR_SAME_ADDRESS);
                    set_atomic_state(mcn, ENTL_STATE_IDLE);
                    set_update_time(mcn, ts);
                }
                else {
                    STM_TDEBUG("Hello message %d received on wait state but not win", u_saddr);
                }
            }
            else if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_EVENT_U) {
                // Hello state got event
                if (l_daddr == 0) {
                    set_i_know(mcn, l_daddr);
                    set_send_next(mcn, l_daddr + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    calc_intervals(mcn);
                    set_update_time(mcn, ts);
                    retval = ENTL_ACTION_SEND;
                    STM_TDEBUG("ENTL %d message received on Hello -> Send", l_daddr);
                }
                else {
                    STM_TDEBUG("Out of sequence ENTL %d message received on Hello", l_daddr);
                }
            }
            else {
                // Received non hello message on Hello state
                STM_TDEBUG("non-hello message %04x received on hello state", u_daddr);
            }
        }
        break;

        case ENTL_STATE_WAIT: {
            if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_HELLO_U) {
                mcn->state_count++;
                if (mcn->state_count > ENTL_COUNT_MAX) {
                    STM_TDEBUG("Hello message %d received overflow %d on Wait state -> Hello state", u_saddr, mcn->state_count);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                }
            }
            else if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_EVENT_U) {
                if (l_daddr == get_i_sent(mcn) + 1) {
                    set_i_know(mcn, l_daddr);
                    set_send_next(mcn, l_daddr + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    set_update_time(mcn, ts);
                    clear_intervals(mcn); 
                    retval = ENTL_ACTION_SEND;
                    STM_TDEBUG("ENTL message %d received on Wait state -> Send state", l_daddr);
                }
                else {
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    clear_intervals(mcn); 
                    STM_TDEBUG("Wrong ENTL message %d received on Wait state -> Hello state", l_daddr);
                }
            }
            else {
                // Received non hello message on Wait state
                STM_TDEBUG("wrong message %04x received on Wait state -> Hello", u_daddr);
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                retval = 0;
            }
        }
        break;

        case ENTL_STATE_SEND: {
            if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_EVENT_U || (u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_ACK_U) {
                if (l_daddr == get_i_know(mcn)) {
                    STM_TDEBUG("Same ENTL message %d received on Send state", l_daddr);
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    retval = ENTL_ACTION_ERROR;
                    STM_TDEBUG("Out of Sequence ENTL %d received on Send state -> Hello", l_daddr);
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                retval = ENTL_ACTION_ERROR;
                STM_TDEBUG("wrong message %04x received on Send state -> Hello", u_daddr);
            }
        }
        break;

        case ENTL_STATE_RECEIVE: {
            if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_EVENT_U) {
                if (get_i_know(mcn) + 2 == l_daddr) {
                    set_i_know(mcn, l_daddr);
                    set_send_next(mcn, l_daddr + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    if (mcn->send_ATI_queue.count) { // AIT has priority
                        retval = ENTL_ACTION_SEND;
                    }
                    else {
                        retval = ENTL_ACTION_SEND | ENTL_ACTION_SEND_DAT; // data send as optional
                    }
                    set_update_time(mcn, ts);
                    //STM_TDEBUG("ETL message %d received on Receive -> Send", l_daddr);
                }
                else if (get_i_know(mcn) == l_daddr) {
                    STM_TDEBUG("same ETL message %d received on Receive", l_daddr);
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    retval = ENTL_ACTION_ERROR;
                    STM_TDEBUG("Out of Sequence ETL message %d received on Receive -> Hello", l_daddr);
                }
            }
            else if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) + 2 == l_daddr) {
                    set_i_know(mcn, l_daddr);
                    set_send_next(mcn, l_daddr + 1);
                    set_atomic_state(mcn, ENTL_STATE_AH);
                    if (ENTT_queue_full(&mcn->receive_ATI_queue)) {
                        STM_TDEBUG("AIT message %d received on Receive with queue full -> Ah", l_daddr);
                        retval = ENTL_ACTION_PROC_AIT;
                    }
                    else {
                        retval = ENTL_ACTION_SEND | ENTL_ACTION_PROC_AIT;
                    }
                    set_update_time(mcn, ts);
                    STM_TDEBUG("AIT message %d received on Receive -> Ah", l_daddr);
                }
                else if (get_i_know(mcn) == l_daddr) {
                    STM_TDEBUG("same ETL message %d received on Receive", l_daddr);
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    retval = ENTL_ACTION_ERROR;
                    STM_TDEBUG("Out of Sequence ETL message %d received on Receive -> Hello", l_daddr);
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                retval = ENTL_ACTION_ERROR;
                STM_TDEBUG("Wrong message %04x received on Receive -> Hello", u_daddr);
            }
        }
        break;

        // AIT 
        case ENTL_STATE_AM: { // AIT message sent, waiting for ack
            if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) + 2 == l_daddr) {
                    set_i_know(mcn, l_daddr);
                    set_send_next(mcn, l_daddr + 1);
                    set_atomic_state(mcn, ENTL_STATE_BM);
                    retval = ENTL_ACTION_SEND;
                    set_update_time(mcn, ts);
                    STM_TDEBUG("ETL Ack %d received on Am -> Bm", l_daddr);
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    retval = ENTL_ACTION_ERROR;
                    STM_TDEBUG("Out of Sequence ETL message %d received on Am -> Hello", l_daddr);
                }
            }
            else if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_EVENT_U) {
                if (get_i_know(mcn) == l_daddr) {
                    STM_TDEBUG("same ETL event %d received on Am", l_daddr);
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    retval = ENTL_ACTION_ERROR;
                    STM_TDEBUG("Wrong message %04x received on Am -> Hello", u_daddr);
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                retval = ENTL_ACTION_ERROR;
                STM_TDEBUG("Wrong message %04x received on Am -> Hello", u_daddr);
            }
        }
        break;

        case ENTL_STATE_BM: { // AIT sent, Ack received, sending Ack
            if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) == l_daddr) {
                    STM_TDEBUG("same ETL Ack %d received on Bm", l_daddr);
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    retval = ENTL_ACTION_ERROR;
                    STM_TDEBUG("Wrong message %04x received on Bm -> Hello", u_daddr);
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                retval = ENTL_ACTION_ERROR;
                STM_TDEBUG("Wrong message %04x received on Bm -> Hello", u_daddr);
            }
        }
        break;

        case ENTL_STATE_AH: { // AIT message received, sending Ack
            if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) == l_daddr) {
                    STM_TDEBUG("Same ENTL message %d received on Ah state", l_daddr);
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    retval = ENTL_ACTION_ERROR;
                    STM_TDEBUG("Out of Sequence ENTL %d received on Ah state -> Hello", l_daddr);
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                retval = ENTL_ACTION_ERROR;
                STM_TDEBUG("wrong message %04x received on Send state -> Hello", u_daddr);
            }
        }
        break;

        case ENTL_STATE_BH: { // got AIT, Ack sent, waiting for ack
            if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_ACK_U) {
                if (get_i_know(mcn) + 2 == l_daddr) {
                    set_i_know(mcn, l_daddr);
                    set_send_next(mcn, l_daddr + 1);
                    set_atomic_state(mcn, ENTL_STATE_SEND);
                    retval = ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT;
                    set_update_time(mcn, ts);
                    STM_TDEBUG("ETL Ack %d received on Bh -> Send", l_daddr);
                    ENTT_queue_back_push(&mcn->receive_ATI_queue, mcn->receive_buffer);
                    mcn->receive_buffer = NULL;
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    retval = ENTL_ACTION_ERROR;
                    STM_TDEBUG("Out of Sequence ETL message %d received on Am -> Hello", l_daddr);
                }
            }
            else if ((u_daddr & ENTL_MESSAGE_MASK) == ENTL_MESSAGE_AIT_U) {
                if (get_i_know(mcn) == l_daddr) {
                    STM_TDEBUG("Same ENTL message %d received on Bh state", l_daddr);
                }
                else {
                    set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                    set_i_know(mcn, 0);
                    set_send_next(mcn, 0);
                    set_i_sent(mcn, 0);
                    set_atomic_state(mcn, ENTL_STATE_HELLO);
                    set_update_time(mcn, ts);
                    retval = ENTL_ACTION_ERROR;
                    STM_TDEBUG("Out of Sequence ENTL %d received on Bh state -> Hello", l_daddr);
                }
            }
            else {
                set_error(mcn, ENTL_ERROR_FLAG_SEQUENCE);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                set_i_sent(mcn, 0);
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                retval = ENTL_ACTION_ERROR;
                STM_TDEBUG("Wrong message %04x received on Am -> Hello", u_daddr);
            }
        }
        break;

        default: {
            STM_TDEBUG("Statemachine on wrong state %d", get_atomic_state(mcn));
            set_error(mcn, ENTL_ERROR_UNKOWN_STATE);
            set_i_know(mcn, 0);
            set_send_next(mcn, 0);
            set_i_sent(mcn, 0);
            set_atomic_state(mcn, ENTL_STATE_IDLE);
            set_update_time(mcn, ts);
        }
        break;
    }
    STM_UNLOCK;
    //STM_TDEBUG("entl_received Statemachine exit on state %d", get_atomic_state(mcn));
    return retval;
}

int entl_get_hello(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr) {
    if (mcn->error_state.error_count) {
        STM_DEBUG("entl_get_hello called on error count set %d", mcn->error_state.error_count);
        return ENTL_ACTION_NOP;
    }

    struct timespec ts = current_kernel_time();
    int ret = ENTL_ACTION_NOP;
    STM_LOCK;
        switch (get_atomic_state(mcn)) {
        case ENTL_STATE_HELLO: {
            *l_addr = ENTL_MESSAGE_HELLO_L; *u_addr = ENTL_MESSAGE_HELLO_U;
            ret = ENTL_ACTION_SEND;
        }
        break;

        case ENTL_STATE_WAIT: {
            //STM_TDEBUG("repeated Message requested on Wait state");
            *l_addr = 0; *u_addr = ENTL_MESSAGE_EVENT_U;
            ret = ENTL_ACTION_SEND;
        }
        break;

        case ENTL_STATE_RECEIVE: {
            STM_TDEBUG("repeated Message requested on Receive state");
            *l_addr = get_i_sent(mcn); *u_addr = ENTL_MESSAGE_EVENT_U;
            ret = ENTL_ACTION_SEND;
        }
        break;

        case ENTL_STATE_AM: {
            STM_TDEBUG("repeated AIT requested on Am state");
            *l_addr = get_i_sent(mcn); *u_addr = ENTL_MESSAGE_AIT_U;
            ret = ENTL_ACTION_SEND | ENTL_ACTION_SEND_AIT;
        }
        break;

        case ENTL_STATE_BH: {
            if (ENTT_queue_full(&mcn->receive_ATI_queue)) {
            }
            else {
                STM_TDEBUG("repeated Ack requested on Bh state");
                *l_addr = get_i_sent(mcn); *u_addr = ENTL_MESSAGE_ACK_U;
                ret = ENTL_ACTION_SEND;
            }
        }
        break;

        default:
        break;
        }
    STM_UNLOCK;
    //STM_TDEBUG("entl_get_hello Statemachine exit on state %d", get_atomic_state(mcn));
    return ret;
}

int entl_next_send(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr) {
    if (mcn->error_state.error_count) {
        *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
        STM_DEBUG("entl_next_send called on error count set %d", mcn->error_state.error_count);
        return ENTL_ACTION_NOP;
    }

    struct timespec ts = current_kernel_time();
    int retval = ENTL_ACTION_NOP;
    STM_LOCK;
        switch (get_atomic_state(mcn)) {
        case ENTL_STATE_IDLE: {
            // say something here as attempt to send something on idle state
            STM_TDEBUG("Message requested on Idle state");
            *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
        }
        break;

        case ENTL_STATE_HELLO: {
            //STM_TDEBUG("repeated Message requested on Hello state");
            *l_addr = ENTL_MESSAGE_HELLO_L; *u_addr = ENTL_MESSAGE_HELLO_U;
            retval = ENTL_ACTION_SEND;
        }
        break;

        case ENTL_STATE_WAIT: {
            //STM_TDEBUG("repeated Message requested on Wait state");
            *l_addr = 0; *u_addr = ENTL_MESSAGE_EVENT_U;
        }
        break;

        case ENTL_STATE_SEND: {
            uint32_t event_i_know = get_i_know(mcn); // last received event number 
            uint32_t event_i_sent = get_i_sent(mcn);
            zebra(mcn);
            advance_send_next(mcn);
            *l_addr = get_i_sent(mcn);
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            // Avoiding to send AIT on the very first loop where other side will be in Hello state
            if (event_i_know && event_i_sent && mcn->send_ATI_queue.count) {
                set_atomic_state(mcn, ENTL_STATE_AM);
                *u_addr = ENTL_MESSAGE_AIT_U;
                retval = ENTL_ACTION_SEND | ENTL_ACTION_SEND_AIT;
                STM_TDEBUG("ETL AIT Message %d requested on Send state -> Am", *l_addr);
            }
            else {
                set_atomic_state(mcn, ENTL_STATE_RECEIVE);
                *u_addr = ENTL_MESSAGE_EVENT_U;
                retval = ENTL_ACTION_SEND | ENTL_ACTION_SEND_DAT; // data send as optional
            }
            //STM_TDEBUG("ETL Message %d requested on Send state -> Receive", *l_addr);
        }
        break;

        case ENTL_STATE_RECEIVE: {
            *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
        }
        break;

        // AIT 
        case ENTL_STATE_AM: {
            *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
        }
        break;

        case ENTL_STATE_BM: {
            struct entt_ioctl_ait_data *ait_data;
            zebra(mcn);
            advance_send_next(mcn);
            *l_addr = get_i_sent(mcn); *u_addr = ENTL_MESSAGE_ACK_U;
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            retval = ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT;
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            // drop the message on the top
            ait_data = ENTT_queue_front_pop(&mcn->send_ATI_queue);
            if (ait_data) {
                kfree(ait_data);
            }
            STM_TDEBUG("ETL AIT ACK %d requested on BM state -> Receive", *l_addr);
        }
        break;

        case ENTL_STATE_AH: {
            if (ENTT_queue_full(&mcn->receive_ATI_queue)) {
                *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
            }
            else {
                zebra(mcn);
                advance_send_next(mcn);
                *l_addr = get_i_sent(mcn); *u_addr = ENTL_MESSAGE_ACK_U;
                calc_intervals(mcn);
                set_update_time(mcn, ts);
                retval = ENTL_ACTION_SEND;
                set_atomic_state(mcn, ENTL_STATE_BH);
                STM_TDEBUG("ETL AIT ACK %d requested on Ah state -> Bh", *l_addr);
            }
        }
        break;

        case ENTL_STATE_BH: {
            *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
        }
        break;

        default: {
            *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
        }
        break; 
        }
    STM_UNLOCK;
    //STM_TDEBUG("entl_next_send Statemachine exit on state %d", get_atomic_state(mcn));
    return retval;
}

// For TX, it can't send AIT, so just keep ENTL state on Send state
int entl_next_send_tx(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr) {
    if (mcn->error_state.error_count) {
        *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
        STM_DEBUG("entl_next_send_tx called on error count set %d", mcn->error_state.error_count);
        return ENTL_ACTION_NOP;
    }

    struct timespec ts = current_kernel_time();
    int retval = ENTL_ACTION_NOP;
    STM_LOCK;
        switch (get_atomic_state(mcn)) {
        case ENTL_STATE_IDLE: {
            // say something here as attempt to send something on idle state
            STM_TDEBUG("Message requested on Idle state");
            *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
        }
        break;
        case ENTL_STATE_HELLO: {
            //STM_TDEBUG("repeated Message requested on Hello state");
            *l_addr = ENTL_MESSAGE_HELLO_L; *u_addr = ENTL_MESSAGE_HELLO_U;
            retval = ENTL_ACTION_SEND;
        }
        break;
        case ENTL_STATE_WAIT: {
            //STM_TDEBUG("repeated Message requested on Wait state");
            *l_addr = 0; *u_addr = ENTL_MESSAGE_EVENT_U;
        }
        break;
        case ENTL_STATE_SEND: {
            zebra(mcn);
            advance_send_next(mcn);
            *l_addr = get_i_sent(mcn);
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            *u_addr = ENTL_MESSAGE_EVENT_U;
            retval = ENTL_ACTION_SEND;
            // For TX, it can't send AIT, so just keep ENTL state on Send state
            //STM_TDEBUG("ETL Message %d requested on Send state -> Receive", *l_addr);
        }
        break;
        case ENTL_STATE_RECEIVE: {
            *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
        }
        break;
        // AIT 
        case ENTL_STATE_AM: {
            *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
        }
        break;
        case ENTL_STATE_BM: {
            zebra(mcn);
            advance_send_next(mcn);
            *l_addr = get_i_sent(mcn); *u_addr = ENTL_MESSAGE_ACK_U;
            calc_intervals(mcn);
            set_update_time(mcn, ts);
            retval = ENTL_ACTION_SEND | ENTL_ACTION_SIG_AIT;
            set_atomic_state(mcn, ENTL_STATE_RECEIVE);
            // drop the message on the top
            ENTT_queue_front_pop(&mcn->send_ATI_queue);
            STM_TDEBUG("ETL AIT ACK %d requested on BM state -> Receive", *l_addr);
        }
        break;
        case ENTL_STATE_AH: {
            if (ENTT_queue_full(&mcn->receive_ATI_queue)) {
                *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
            }
            else {
                zebra(mcn);
                advance_send_next(mcn);
                *l_addr = get_i_sent(mcn); *u_addr = ENTL_MESSAGE_ACK_U;
                calc_intervals(mcn);
                set_update_time(mcn, ts);
                retval = ENTL_ACTION_SEND;
                set_atomic_state(mcn, ENTL_STATE_BH);
                STM_TDEBUG("ETL AIT ACK %d requested on Ah state -> Bh", *l_addr);
            }
        }
        break;
        case ENTL_STATE_BH: {
            *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
        }
        break;
        default: {
            *l_addr = 0; *u_addr = ENTL_MESSAGE_NOP_U;
        }
        break; 
        }
    STM_UNLOCK;
    //STM_TDEBUG("entl_next_send Statemachine exit on state %d", get_atomic_state(mcn));
    return retval;
}

void entl_state_error(entl_state_machine_t *mcn, uint32_t error_flag) {
    if (error_flag == ENTL_ERROR_FLAG_LINKDONW && get_atomic_state(mcn) == ENTL_STATE_IDLE) return;

    struct timespec ts = current_kernel_time();
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
    STM_TDEBUG("entl_state_error %d Statemachine exit on state %d", error_flag, get_atomic_state(mcn));
}

void entl_read_current_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err) {
    struct timespec ts = current_kernel_time();
    STM_LOCK;
        memcpy(st, &mcn->current_state, sizeof(entl_state_t));
        memcpy(err, &mcn->error_state, sizeof(entl_state_t));
    STM_UNLOCK;
    //STM_TDEBUG("entl_read_current_state Statemachine exit on state %d", get_atomic_state(mcn));
}

void entl_read_error_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err) {
    struct timespec ts = current_kernel_time();
    STM_LOCK;
        memcpy(st, &mcn->current_state, sizeof(entl_state_t));
        memcpy(err, &mcn->error_state, sizeof(entl_state_t));
        memset(&mcn->error_state, 0, sizeof(entl_state_t));
    STM_UNLOCK;
    //STM_TDEBUG("entl_read_error_state Statemachine exit on state %d", get_atomic_state(mcn));
}

void entl_link_up(entl_state_machine_t *mcn) {
    struct timespec ts = current_kernel_time();
    STM_LOCK;
        if (get_atomic_state(mcn) == ENTL_STATE_IDLE) {
            if (mcn->error_state.error_count) {
                STM_TDEBUG("got Link UP with error count %d ignored", mcn->error_state.error_count);
            }
            else {
                STM_TDEBUG("Link UP !!");
                set_atomic_state(mcn, ENTL_STATE_HELLO);
                set_update_time(mcn, ts);
                clear_error(mcn);
                // when following 3 members are all zero, it means fresh out of Hello handshake
                set_i_sent(mcn, 0);
                set_i_know(mcn, 0);
                set_send_next(mcn, 0);
                clear_intervals(mcn);
            }
        }
        else {
            STM_TDEBUG("Unexpected Link UP on state %d ignored", get_atomic_state(mcn));
            //set_error(mcn, ENTL_ERROR_UNEXPECTED_LU);
            //set_atomic_state(mcn, ENTL_STATE_HELLO);
            //set_update_time(mcn, ts);
        }
    STM_UNLOCK;
    //STM_TDEBUG("entl_link_up Statemachine exit on state %d", get_atomic_state(mcn));
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
        uint16_t ret = mcn->send_ATI_queue.count;
    STM_UNLOCK;
    return ret;
}
