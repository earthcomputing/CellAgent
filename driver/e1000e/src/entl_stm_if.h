#ifndef _ENTL_STM_IF_H_
#define _ENTL_STM_IF_H_

extern int entl_get_hello(entl_state_machine_t *p, uint16_t *emsg_raw, uint32_t *seqno);
extern int entl_next_send(entl_state_machine_t *p, uint16_t *emsg_raw, uint32_t *seqno); // ENTL_ACTION
extern int entl_next_send_tx(entl_state_machine_t *p, uint16_t *emsg_raw, uint32_t *seqno); // ENTL_ACTION
extern int entl_received(entl_state_machine_t *p, uint16_t from_hi, uint32_t from_lo, uint16_t emsg_raw, uint32_t seqno); // ENTL_ACTION
extern int entl_send_AIT_message(entl_state_machine_t *p, struct entt_ioctl_ait_data *data);
extern struct entt_ioctl_ait_data *entl_next_AIT_message(entl_state_machine_t *p);
extern struct entt_ioctl_ait_data *entl_read_AIT_message(entl_state_machine_t *p); 
extern uint16_t entl_num_queued(entl_state_machine_t *p);
extern void entl_link_up(entl_state_machine_t *p);
extern void entl_new_AIT_message(entl_state_machine_t *p, struct entt_ioctl_ait_data *data);
extern void entl_read_current_state(entl_state_machine_t *p, entl_state_t *st, entl_state_t *err);
extern void entl_read_error_state(entl_state_machine_t *p, entl_state_t *st, entl_state_t *err);
extern void entl_set_my_adder(entl_state_machine_t *p, uint16_t mac_hi, uint32_t mac_lo); 
extern void entl_state_error(entl_state_machine_t *p, uint32_t error_flag); // enter error state

extern void dump_ait_data(entl_state_machine_t *stm, char *tag, struct entt_ioctl_ait_data *ait_data);

#endif
