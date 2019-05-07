#include <linux/module.h>
#include <linux/types.h>

#include "entl_state_machine.h"

#if 0
void entl_link_up(entl_state_machine_t *mcn);
int entl_next_send(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr); // ENTL_ACTION
int entl_next_send_tx(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr); // ENTL_ACTION
int entl_received(entl_state_machine_t *mcn, uint16_t u_saddr, uint32_t l_saddr, uint16_t u_daddr, uint32_t l_daddr); // ENTL_ACTION
void entl_state_error(entl_state_machine_t *mcn, uint32_t error_flag); // enter error state

void entl_new_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data);
int entl_send_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data);
struct entt_ioctl_ait_data *entl_next_AIT_message(entl_state_machine_t *mcn);
struct entt_ioctl_ait_data *entl_read_AIT_message(entl_state_machine_t *mcn); 

void entl_read_current_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err);
void entl_read_error_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err);

uint16_t entl_num_queued(entl_state_machine_t *mcn);

void entl_state_machine_init(entl_state_machine_t *mcn);
void entl_set_my_adder(entl_state_machine_t *mcn, u_addr u_addr, uint32_t l_addr); 

// algorithm:
int entl_get_hello(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr);

#endif
