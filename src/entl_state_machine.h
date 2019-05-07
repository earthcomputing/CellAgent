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
#define ENTL_MESSAGE_HELLO_L 0x0000
#define ENTL_MESSAGE_EVENT_U 0x0001
#define ENTL_MESSAGE_NOP_U   0x0002
#define ENTL_MESSAGE_AIT_U   0x0003
#define ENTL_MESSAGE_ACK_U   0x0004
#define ENTL_MESSAGE_MASK    0x00ff
#define ENTL_MESSAGE_ONLY_U  0x8000
#define ENTL_TEST_MASK       0x7f00

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

// uint32_t entl_state = FETCH_STATE(stm);

#define ENTL_DEVICE_NAME_LEN 15
typedef struct entl_state_machine {
} entl_state_machine_t;

void entl_link_up(entl_state_machine_t *mcn);
int entl_next_send(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr); // ENTL_ACTION
int entl_next_send_tx(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr); // ENTL_ACTION
int entl_received(entl_state_machine_t *mcn, uint16_t u_saddr, uint32_t l_saddr, uint16_t u_daddr, uint32_t l_daddr); // ENTL_ACTION
void entl_state_error(entl_state_machine_t *mcn, uint32_t error_flag); // enter error state

#include "entl_user_api.h"

void entl_new_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data);
int entl_send_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data);
struct entt_ioctl_ait_data *entl_next_AIT_message(entl_state_machine_t *mcn);
struct entt_ioctl_ait_data *entl_read_AIT_message(entl_state_machine_t *mcn); 

#include "entl_state.h"

void entl_read_current_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err);
void entl_read_error_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err);

uint16_t entl_num_queued(entl_state_machine_t *mcn);

void entl_state_machine_init(entl_state_machine_t *mcn);
void entl_set_my_adder(entl_state_machine_t *mcn, uint16_t u_addr, uint32_t l_addr); 

// algorithm:
int entl_get_hello(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr);

#endif
