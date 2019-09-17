#ifndef _ECNL_SDK_H_
#define _ECNL_SDK_H_

//#include "ecnl_user_api.h"

#include "ecnl_proto.h"

int alloc_ecnl_session(void **ecnl_session_ptr);
int ecnl_get_module_info(void *ecnl_session, const module_info_t **mipp);
int ecnl_get_port_state(void *ecnl_session, uint32_t port_id, const link_state_t **lspp);
int ecnl_alloc_table(void *ecnl_session, uint32_t table_size, const uint32_t **table_idp);
int ecnl_dealloc_table(void *ecnl_session, uint32_t table_id);
int ecnl_select_table(void *ecnl_session, uint32_t table_id);
int ecnl_fill_table(void *ecnl_session, uint32_t table_id, uint32_t table_location, uint32_t table_content_size, ecnl_table_entry_t *table_content);
int ecnl_fill_table_entry(void *ecnl_session, uint32_t table_id, uint32_t table_location, ecnl_table_entry_t *table_entry);
int ecnl_map_ports(void *ecnl_session, uint32_t **table_mapp);
int ecnl_start_forwarding(void *ecnl_session);
int ecnl_stop_forwarding(void *ecnl_session);
int ecnl_read_alo_register(void *ecnl_session, uint32_t port_id, uint32_t alo_reg_no, uint64_t *alo_reg_data_p);
int ecnl_retrieve_ait_message(void *ecnl_session, uint32_t port_id, buf_desc_t **buf_desc);
int ecnl_write_alo_register(void *ecnl_session, uint32_t port_id, uint32_t alo_reg_no, uint64_t alo_reg_data);
int ecnl_send_ait_message(void *ecnl_session, uint32_t port_id, buf_desc_t buf_desc);
int ecnl_send_discover_message(void *ecnl_session, uint32_t port_id, buf_desc_t buf_desc);
int ecnl_signal_ait_message(void *ecnl_session, uint32_t port_id, buf_desc_t buf_desc);
int free_ecnl_session(void *ecnl_session);

#endif
