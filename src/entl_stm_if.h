#ifndef _ENTL_STM_IF_H_
#define _ENTL_STM_IF_H_

extern int entl_get_hello(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr);
extern int entl_next_send(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr); // ENTL_ACTION
extern int entl_next_send_tx(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr); // ENTL_ACTION
extern int entl_received(entl_state_machine_t *mcn, uint16_t u_saddr, uint32_t l_saddr, uint16_t u_daddr, uint32_t l_daddr); // ENTL_ACTION
extern int entl_send_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data);
extern struct entt_ioctl_ait_data *entl_next_AIT_message(entl_state_machine_t *mcn);
extern struct entt_ioctl_ait_data *entl_read_AIT_message(entl_state_machine_t *mcn); 
extern uint16_t entl_num_queued(entl_state_machine_t *mcn);
extern void entl_link_up(entl_state_machine_t *mcn);
extern void entl_new_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data);
extern void entl_read_current_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err);
extern void entl_read_error_state(entl_state_machine_t *mcn, entl_state_t *st, entl_state_t *err);
extern void entl_set_my_adder(entl_state_machine_t *mcn, uint16_t u_addr, uint32_t l_addr); 
extern void entl_state_error(entl_state_machine_t *mcn, uint32_t error_flag); // enter error state

#endif
