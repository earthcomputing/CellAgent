#include "ecnl_user_api.h"

typedef struct {
    uint32_t module_id;
    char *module_name;
    uint32_t num_ports;
} module_info_t;

typedef struct {
    char *module_name;
    char *port_name;
    uint32_t port_link_state;
    uint64_t port_s_counter;
    uint64_t port_r_counter;
    uint64_t port_recover_counter;
    uint64_t port_recovered_counter;
    uint64_t port_entt_count;
    uint64_t port_aop_count;
    uint32_t num_ait_messages;
} link_state_t;

typedef struct {
    uint32_t ar_no;
    uint64_t ar_data;
} alo_reg_t;

typedef struct {
    uint32_t len;
    uint8_t *frame;
} buf_desc_t;

typedef struct {
    uint32_t magic;
    struct nlattr *tb[NL_ECNL_ATTR_MAX+1];
} callback_index_t;

int alloc_nl_session(void **nl_session_ptr);
int get_module_info(void *nl_session, const module_info_t **mipp);
int get_port_state(void *nl_session, uint32_t port_id, uint32_t *mp, uint32_t *pp, link_state_t *lp);
int alloc_table(void *nl_session, uint32_t table_size, uint32_t *mp, uint32_t *tp);
int dealloc_table(void *nl_session, uint32_t table_id, uint32_t *mp, uint32_t *tp);
int select_table(void *nl_session, uint32_t table_id, uint32_t *mp, uint32_t *tp);
int fill_table(void *nl_session, uint32_t table_id, uint32_t table_location, uint32_t table_content_size, ecnl_table_entry_t *table_content, uint32_t *mp, uint32_t *tp);
int fill_table_entry(void *nl_session, uint32_t table_id, uint32_t table_location, ecnl_table_entry_t *table_entry, uint32_t *mp, uint32_t *tp);
int map_ports(void *nl_session, uint32_t *table_map, uint32_t *mp);
int start_forwarding(void *nl_session, uint32_t *mp);
int stop_forwarding(void *nl_session, uint32_t *mp);
int read_alo_registers(void *nl_session, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp, uint32_t *fp, uint64_t **vp);
int retrieve_ait_message(void *nl_session, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp, buf_desc_t *buf);
int write_alo_register(void *nl_session, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp);
int send_ait_message(void *nl_session, uint32_t port_id, buf_desc_t buf, uint32_t *mp, uint32_t *pp);
int event_receive_dsc(void **tbv, uint32_t *mp, uint32_t *pp, uint8_t *dp);
int event_link_status_update(void **tbv, uint32_t *mp, uint32_t *pp, link_state_t *lp);
int event_forward_ait_message(void **tbv, uint32_t *mp, uint32_t *pp, uint32_t *lp, uint8_t *dp);
int event_got_ait_message(void **tbv, uint32_t *mp, uint32_t *pp, uint32_t *lp);
int event_got_alo_update(void **tbv, uint32_t *mp, uint32_t *pp, uint64_t *vp, uint32_t *fp);
int send_discover_message(void *nl_session, uint32_t port_id, buf_desc_t buf);
int signal_ait_message(void *nl_session, uint32_t port_id, buf_desc_t buf, uint32_t *mp, uint32_t *pp);
int free_nl_session(void *nl_session);

// --

#define ECNL_GENL_VERSION 0x0000 // "0.0.0.2"

#define ARRAY_SIZE(X) (sizeof(X) / sizeof((X)[0]))

static void dump_msg(void *user_hdr);

#define __ADD(id, name) { .i = id, .a = #name }

typedef struct {
    uint64_t i;
    const char *a;
} trans_tbl_t;

