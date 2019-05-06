#include <linux/module.h>
#include <linux/types.h>

#include "entl_state_machine.h"

#if 0
int entl_next_send(entl_state_machine_t *mcn, uint16_t *u_addr, uint32_t *l_addr); // ENTL_ACTION
int entl_received(entl_state_machine_t *mcn, uint16_t u_saddr, uint32_t l_saddr, uint16_t u_daddr, uint32_t l_daddr); // ENTL_ACTION
void entl_state_error(entl_state_machine_t *mcn, uint32_t error_flag); // enter error state

void entl_new_AIT_message(entl_state_machine_t *mcn, struct entt_ioctl_ait_data *data);
struct entt_ioctl_ait_data *entl_next_AIT_message(entl_state_machine_t *mcn);
#endif
