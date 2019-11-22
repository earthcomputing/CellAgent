#ifndef _ECNL_SDK_H_
#define _ECNL_SDK_H_

//#include "ecnl_user_api.h"

#include "ecnl_proto.h"

int alloc_nl_session(void **nl_session_ptr);
int ecnl_get_module_info(void *nl_session_void, const module_info_t **mipp);
int ecnl_alloc_table(void *nl_session_void, uint32_t table_size, const uint32_t **table_idp);
int ecnl_dealloc_table(void *nl_session_void, uint32_t table_id);
int ecnl_select_table(void *nl_session_void, uint32_t table_id);
int ecnl_fill_table(void *nl_session_void, uint32_t table_id, uint32_t table_location, uint32_t table_content_size, ecnl_table_entry_t *table_content);
int ecnl_fill_table_entry(void *nl_session_void, uint32_t table_id, uint32_t table_location, ecnl_table_entry_t *table_entry);
int ecnl_map_ports(void *nl_session_void, uint32_t **table_mapp);
int ecnl_start_forwarding(void *nl_session_void);
int ecnl_stop_forwarding(void *nl_session_void);
int free_nl_session(void *nl_session_void);

#endif
