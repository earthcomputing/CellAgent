#ifndef _ECNL_PROTO_H_
#define _ECNL_PROTO_H_

#include <netlink/cli/utils.h>
#include <linux/genetlink.h>

// #include "ecnl_user_api.h"

// #include <e1000.h>
// #include <ecnl_entl_if.h>
// #include <ecnl_device.h>
#include "ecnl_table.h"
#include <ecnl_protocol.h>


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

extern struct nl_sock *init_sock();

extern int get_module_info(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, module_info_t *mip);
extern int get_port_state(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, uint32_t *mp, uint32_t *pp, link_state_t *lp);
extern int alloc_driver(struct nl_sock *sock, struct nl_msg *msg, char *module_name, uint32_t *mp);
extern int alloc_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_size, uint32_t *mp, uint32_t *tp);
extern int dealloc_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t *mp, uint32_t *tp);
extern int select_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t *mp, uint32_t *tp);
extern int fill_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t table_location, uint32_t table_content_size, ecnl_table_entry_t *table_content, uint32_t *mp, uint32_t *tp);
extern int fill_table_entry(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t table_location, ecnl_table_entry_t *table_entry, uint32_t *mp, uint32_t *tp);
extern int map_ports(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t *table_map, uint32_t *mp);
extern int start_forwarding(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t *mp);
extern int stop_forwarding(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t *mp);
extern int read_alo_registers(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp, uint32_t *fp, uint64_t **vp);
extern int retrieve_ait_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp, buf_desc_t *buf);
extern int write_alo_register(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp);
extern int send_ait_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, buf_desc_t buf, uint32_t *mp, uint32_t *pp);
extern int event_receive_dsc(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint8_t *dp);
extern int event_link_status_update(struct nlattr **tb, uint32_t *mp, uint32_t *pp, link_state_t *lp);
extern int event_forward_ait_message(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint32_t *lp, uint8_t *dp);
extern int event_got_ait_massage(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint32_t *lp);
extern int event_got_alo_update(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint64_t *vp, uint32_t *fp);
extern int send_discover_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, buf_desc_t buf);
extern int signal_ait_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, buf_desc_t buf, uint32_t *mp, uint32_t *pp);

// --

#define ECNL_GENL_VERSION 0x0000 // "0.0.0.2"

extern void dump_msg(void *user_hdr);
extern void fatal_error(int err, const char *fmt, ...);

#define ARRAY_SIZE(X) (sizeof(X) / sizeof((X)[0]))

#endif
