#include <netlink/cli/utils.h>

#include "ecnl_user_api.h"
#include <linux/genetlink.h>
// #include <net/genetlink.h> - kernel only!

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

static int get_module_info(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, module_info_t *mip);
static int get_port_state(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, uint32_t *mp, uint32_t *pp, link_state_t *lp);
static int alloc_driver(struct nl_sock *sock, struct nl_msg *msg, char *module_name, uint32_t *mp);
static int alloc_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_size, uint32_t *mp, uint32_t *tp);
static int dealloc_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t *mp, uint32_t *tp);
static int select_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t *mp, uint32_t *tp);
static int fill_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t table_location, uint32_t table_content_size, ecnl_table_entry_t *table_content, uint32_t *mp, uint32_t *tp);
static int fill_table_entry(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t table_location, ecnl_table_entry_t *table_entry, uint32_t *mp, uint32_t *tp);
static int map_ports(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t *table_map, uint32_t *mp);
static int start_forwarding(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t *mp);
static int stop_forwarding(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t *mp);
static int read_alo_registers(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp, uint32_t *fp, uint64_t **vp);
static int retrieve_ait_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp, buf_desc_t *buf);
static int write_alo_register(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp);
static int send_ait_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, buf_desc_t buf, uint32_t *mp, uint32_t *pp);
static int event_receive_dsc(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint8_t *dp);
static int event_link_status_update(struct nlattr **tb, uint32_t *mp, uint32_t *pp, link_state_t *lp);
static int event_forward_ait_message(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint32_t *lp, uint8_t *dp);
static int event_got_ait_massage(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint32_t *lp);
static int event_got_alo_update(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint64_t *vp, uint32_t *fp);
static int send_discover_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, buf_desc_t buf);
static int signal_ait_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, buf_desc_t buf, uint32_t *mp, uint32_t *pp);

// --

#define ECNL_GENL_VERSION 0x0000 // "0.0.0.2"

#define ARRAY_SIZE(X) (sizeof(X) / sizeof((X)[0]))

static void dump_msg(void *user_hdr);

#define __ADD(id, name) { .i = id, .a = #name }

typedef struct {
    uint64_t i;
    const char *a;
} trans_tbl_t;

static const trans_tbl_t attr_names[] = {
    __ADD(NL_ECNL_ATTR_MODULE_NAME, module_name),
    __ADD(NL_ECNL_ATTR_MODULE_ID, module_id),
    __ADD(NL_ECNL_ATTR_NUM_PORTS, num_ports),
    __ADD(NL_ECNL_ATTR_PORT_ID, port_id),
    __ADD(NL_ECNL_ATTR_PORT_NAME, port_name),
    __ADD(NL_ECNL_ATTR_PORT_LINK_STATE, port_link_state),
    __ADD(NL_ECNL_ATTR_PORT_S_COUNTER, port_s_counter),
    __ADD(NL_ECNL_ATTR_PORT_R_COUNTER, port_r_counter),
    __ADD(NL_ECNL_ATTR_PORT_RECOVER_COUNTER, port_recover_counter),
    __ADD(NL_ECNL_ATTR_PORT_RECOVERED_COUNTER, port_recovered_counter),
    __ADD(NL_ECNL_ATTR_PORT_ENTT_COUNT, port_entt_count),
    __ADD(NL_ECNL_ATTR_PORT_AOP_COUNT, port_aop_count),
    __ADD(NL_ECNL_ATTR_NUM_AIT_MESSAGES, num_ait_messages),
    __ADD(NL_ECNL_ATTR_TABLE_SIZE, table_size),
    __ADD(NL_ECNL_ATTR_TABLE_ID, table_id),
    __ADD(NL_ECNL_ATTR_TABLE_LOCATION, table_location),
    __ADD(NL_ECNL_ATTR_TABLE_CONTENT, table_content),
    __ADD(NL_ECNL_ATTR_TABLE_CONTENT_SIZE, table_content_size),
    __ADD(NL_ECNL_ATTR_TABLE_ENTRY, table_entry),
    __ADD(NL_ECNL_ATTR_TABLE_ENTRY_LOCATION, table_entry_location),
    __ADD(NL_ECNL_ATTR_TABLE_MAP, table_map),
    __ADD(NL_ECNL_ATTR_MESSAGE, message),
    __ADD(NL_ECNL_ATTR_DISCOVERING_MSG, discovering_msg),
    __ADD(NL_ECNL_ATTR_MESSAGE_LENGTH, message_length),
    __ADD(NL_ECNL_ATTR_ALO_REG_VALUES, alo_reg_values),
    __ADD(NL_ECNL_ATTR_ALO_FLAG, alo_flag),
    __ADD(NL_ECNL_ATTR_ALO_REG_DATA, alo_reg_data),
    __ADD(NL_ECNL_ATTR_ALO_REG_NO, alo_reg_no),
};

static struct nla_policy attr_policy[NL_ECNL_ATTR_MAX+1] = {
    [NL_ECNL_ATTR_MODULE_NAME] =                { .type = NLA_NUL_STRING, .maxlen = 20-1 },
    [NL_ECNL_ATTR_MODULE_ID] =                  { .type = NLA_U32 },
    [NL_ECNL_ATTR_NUM_PORTS] =                  { .type = NLA_U32 },
    [NL_ECNL_ATTR_PORT_ID] =                    { .type = NLA_U32 },
    [NL_ECNL_ATTR_PORT_NAME] =                  { .type = NLA_NUL_STRING, .maxlen = 20-1 },
    [NL_ECNL_ATTR_PORT_LINK_STATE] =            { .type = NLA_U32 },
    [NL_ECNL_ATTR_PORT_S_COUNTER] =             { .type = NLA_U64 },
    [NL_ECNL_ATTR_PORT_R_COUNTER] =             { .type = NLA_U64 },
    [NL_ECNL_ATTR_PORT_RECOVER_COUNTER] =       { .type = NLA_U64 },
    [NL_ECNL_ATTR_PORT_RECOVERED_COUNTER] =     { .type = NLA_U64 },
    [NL_ECNL_ATTR_PORT_ENTT_COUNT] =            { .type = NLA_U64 },
    [NL_ECNL_ATTR_PORT_AOP_COUNT] =             { .type = NLA_U64 },
    [NL_ECNL_ATTR_NUM_AIT_MESSAGES] =           { .type = NLA_U32 },
    [NL_ECNL_ATTR_TABLE_SIZE] =                 { .type = NLA_U32 },
    [NL_ECNL_ATTR_TABLE_ID] =                   { .type = NLA_U32 },
    [NL_ECNL_ATTR_TABLE_LOCATION] =             { .type = NLA_U32 },
    [NL_ECNL_ATTR_TABLE_CONTENT] =              { .type = NL_ECNL_ATTR_UNSPEC },
    [NL_ECNL_ATTR_TABLE_CONTENT_SIZE] =         { .type = NLA_U32 },
    [NL_ECNL_ATTR_TABLE_ENTRY] =                { .type = NL_ECNL_ATTR_UNSPEC },
    [NL_ECNL_ATTR_TABLE_ENTRY_LOCATION] =       { .type = NLA_U32 },
    [NL_ECNL_ATTR_TABLE_MAP] =                  { .type = NL_ECNL_ATTR_UNSPEC },
    [NL_ECNL_ATTR_MESSAGE] =                    { .type = NL_ECNL_ATTR_UNSPEC },
    [NL_ECNL_ATTR_DISCOVERING_MSG] =            { .type = NL_ECNL_ATTR_UNSPEC },
    [NL_ECNL_ATTR_MESSAGE_LENGTH] =             { .type = NLA_U32 },
    [NL_ECNL_ATTR_ALO_REG_VALUES] =             { .type = NL_ECNL_ATTR_UNSPEC },
    [NL_ECNL_ATTR_ALO_FLAG] =                   { .type = NLA_U32 },
    [NL_ECNL_ATTR_ALO_REG_DATA] =               { .type = NLA_U64 },
    [NL_ECNL_ATTR_ALO_REG_NO] =                 { .type = NLA_U32 }
};

// man stdarg
void fatal_error(int err, const char *fmt, ...) {
    va_list ap;
    fprintf(stderr, "Error: %s - ", strerror(err));
    va_start(ap, fmt);
    vfprintf(stderr, fmt, ap);
    va_end(ap);
    fprintf(stderr, "\n");
    exit(abs(err));
}

static void dump_block(void *d, int nbytes) {
    printf("nbytes: %d\n        ", nbytes);
    for (int i = 0; i < nbytes; i++) {
        printf("%02x", ((char *) d)[i] & 0xff);
        if (i % 16 == 15) printf("\n        ");
    }
    printf("\n");
}

// .genlhdr
// .userhdr
// nl_cb_action - NL_OK, NL_SKIP, NL_STOP
static int parse_generic(struct nl_cache_ops *unused, struct genl_cmd *cmd, struct genl_info *info, void *arg) {
    callback_index_t *cbi = (callback_index_t *) arg;
    if (cbi->magic != 0x5a5a) fatal_error(-1, "garbled callback arg");
    struct nlmsghdr *nlh = info->nlh;

    printf("parse_generic:\n");

    if (nlh->nlmsg_type == NLMSG_ERROR) { printf("NLMSG_ERROR\n"); return -1; } // FIXME: should this be NL_OK ??

    int err = genlmsg_parse(nlh, 0, &cbi->tb[0], NL_ECNL_ATTR_MAX, attr_policy);
    if (err < 0) { printf("genlmsg_parse error\n"); return NL_SKIP; } // FIXME: what return code here ??

    for (int i = 0; i < ARRAY_SIZE(attr_names); i++) {
        const trans_tbl_t *tp = &attr_names[i];
        uint64_t attr = tp->i;
        struct nlattr *na = cbi->tb[attr];
        if (na == NULL) continue;
        struct nla_policy *pp = &attr_policy[attr];
        switch (pp->type) {
        // NLA_FLAG NLA_U8 NLA_U16 NLA_MSECS NLA_STRING NLA_UNSPEC NLA_NESTED
        case NLA_U32: printf("%s(%ld): %d\n", tp->a, attr, nla_get_u32(na)); continue;
        case NLA_U64: printf("%s(%ld): %ld\n", tp->a, attr, nla_get_u64(na)); continue;
        case NLA_NUL_STRING: printf("%s(%ld): \"%s\"\n", tp->a, attr, nla_get_string(na)); continue;
        case NL_ECNL_ATTR_UNSPEC: printf("%s(%ld)\n", tp->a, attr); dump_block(nla_data(na), nla_len(na)); continue;
        }
        printf("%s (%ld)\n", tp->a, attr);
    }
    return NL_OK;
}

#if 0
static int parse_cmd_new(struct nl_cache_ops *unused, struct genl_cmd *cmd, struct genl_info *info, void *arg) {
    struct nlattr *attrs[NL_ECNL_ATTR_MAX+1];

    struct nlattr *nested;
    if (info->attrs[NL_ECNL_ATTR_TABLE_ENTRY]) {
        nested = info->attrs[NL_ECNL_ATTR_TABLE_ENTRY];
    }
    else {
        fprintf(stderr, "Invalid yy message: Unable to find nested attribute/\n");
        return NL_SKIP;
    }

    int err = nla_parse_nested(attrs, NL_ECNL_ATTR_MAX, nested, attr_policy);
    if (err < 0) {
        nl_perror(err, "Error while parsing generic netlink message");
        return err;
    }

/*
    if (attrs[xx]) {
        struct yy *yy = nla_data(attrs[xx]);
        printf("%s pid %u uid %u gid %u parent %u\n", yy->ac_comm, yy->ac_pid, yy->ac_uid, yy->ac_gid, yy->ac_ppid);
    }
*/

    return 0;
}
#endif

// ref: lib/genl/mngt.c
static int parse_cb(struct nl_msg *msg, void *arg) {
    return genl_handle_msg(msg, arg);
}

static struct genl_cmd cmds[] = {
    { .c_id = NL_ECNL_CMD_ALLOC_DRIVER, .c_name = "alloc_driver()",
        .c_maxattr = NL_ECNL_ATTR_MAX,
        .c_msg_parser = &parse_generic,
        .c_attr_policy = attr_policy,
    },
    { .c_id = NL_ECNL_CMD_GET_MODULE_INFO, .c_name = "get_module_info()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_GET_PORT_STATE, .c_name = "get_port_state()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_ALLOC_TABLE, .c_name = "alloc_table()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_FILL_TABLE, .c_name = "fill_table()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_FILL_TABLE_ENTRY, .c_name = "fill_table_entry()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_SELECT_TABLE, .c_name = "select_table()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_DEALLOC_TABLE, .c_name = "dealloc_table()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_MAP_PORTS, .c_name = "map_ports()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_START_FORWARDING, .c_name = "start_forwarding()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_STOP_FORWARDING, .c_name = "stop_forwarding()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_SEND_AIT_MESSAGE, .c_name = "send_ait_message()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_SIGNAL_AIT_MESSAGE, .c_name = "signal_ait_message()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_RETRIEVE_AIT_MESSAGE, .c_name = "retrieve_ait_message()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_WRITE_ALO_REGISTER, .c_name = "write_alo_register()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_READ_ALO_REGISTERS, .c_name = "read_alo_registers()", .c_msg_parser = &parse_generic },
    { .c_id = NL_ECNL_CMD_SEND_DISCOVER_MESSAGE, .c_name = "send_discover_message()", .c_msg_parser = &parse_generic },
};


// static struct nl_cache_ops genl_ecnl_cache_ops = { ... };

static struct genl_ops ops = {
    .o_name = ECNL_GENL_NAME,
    .o_cmds = cmds,
    .o_ncmds = ARRAY_SIZE(cmds),
    // .o_hdrsize = 0,
    // .o_id = 0, // from genl_ops_resolve
    // .o_cache_ops = NULL,
    // .o_list = NULL,
};

#define WAIT_ACK { int err = nl_wait_for_ack(sock); if (err < 0) fatal_error(err, "no ack?"); }

int doit(struct nl_sock *sock, struct nl_msg *msg) {
    uint32_t module_id = 0;

// --

#if 0
int sim_module_id;
    {
printf("alloc_driver\n");
        char *module_name = "sim_ecnl0";
        int rc = alloc_driver(sock, msg, module_name, &sim_module_id);
        if (rc < 0) fatal_error(rc, "alloc_driver");
    }
#endif

// --

uint32_t num_ports = -1;

#define CLEAR_MSG { nlmsg_free(msg); msg = nlmsg_alloc(); }
CLEAR_MSG;
    {
printf("get_module_info\n");
        module_info_t mi; memset(&mi, 0, sizeof(module_info_t));
        int rc = get_module_info(sock, msg, module_id, &mi);
        if (rc < 0) fatal_error(rc, "get_module_info");
num_ports = mi.num_ports;
    }

// --

    uint32_t actual_module_id;
CLEAR_MSG;
    {
printf("start_forwarding\n");
        int rc = start_forwarding(sock, msg, module_id, &actual_module_id);
        if (rc < 0) fatal_error(rc, "start_forwarding");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    }

CLEAR_MSG;
    {
printf("stop_forwarding\n");
        int rc = stop_forwarding(sock, msg, module_id, &actual_module_id);
        if (rc < 0) fatal_error(rc, "stop_forwarding");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    }

// --

    uint32_t table_id;
CLEAR_MSG;
    {
printf("alloc_table\n");
        uint32_t table_size = 1000;
        int rc = alloc_table(sock, msg, module_id, table_size, &actual_module_id, &table_id);
        if (rc < 0) fatal_error(rc, "alloc_table");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    }

#if 0

#define ENCL_FW_TABLE_ENTRY_ARRAY 15
typedef struct ecnl_table_entry {
    union {
        uint32_t raw_vector;
        struct {
            unsigned int reserved: 12;
            unsigned int parent: 4;
            unsigned int port_vector: 16;
        };
    } info;
    uint32_t nextID[ENCL_FW_TABLE_ENTRY_ARRAY];
} ecnl_table_entry_t;

    char *p = (char *) &ecnl_table[location];
    nla_memcpy(p, info->attrs[NL_ECNL_ATTR_TABLE_CONTENT], sizeof(struct ecnl_table_entry) * size);
#endif

    uint32_t actual_table_id;
CLEAR_MSG;
    {
printf("fill_table\n");
        ecnl_table_entry_t table_content[] = {
            {
                .info = { .parent = 3, .port_vector = 0x0002, },
                .nextID = { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14 },
            },
        };
        uint32_t table_content_size = ARRAY_SIZE(table_content);
        uint32_t table_location = 42;
        int rc = fill_table(sock, msg, module_id, table_id, table_location, table_content_size, &table_content[0], &actual_module_id, &actual_table_id);
        if (rc < 0) fatal_error(rc, "fill_table");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_table_id != table_id) fatal_error(-1, "table mismatch: %d, %d", table_id, actual_table_id);
    }

CLEAR_MSG;
    {
printf("fill_table_entry\n");
        ecnl_table_entry_t table_entry = {
            .info = { .parent = 3, .port_vector = 0x0002, },
            .nextID = { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14 },
        };
        uint32_t table_location = 43;
        int rc = fill_table_entry(sock, msg, module_id, table_id, table_location, &table_entry, &actual_module_id, &actual_table_id);
        if (rc < 0) fatal_error(rc, "fill_table_entry");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_table_id != table_id) fatal_error(-1, "table mismatch: %d, %d", table_id, actual_table_id);
    }

CLEAR_MSG;
    {
printf("select_table\n");
        int rc = select_table(sock, msg, module_id, table_id, &actual_module_id, &actual_table_id);
        if (rc < 0) fatal_error(rc, "select_table");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_table_id != table_id) fatal_error(-1, "table mismatch: %d, %d", table_id, actual_table_id);
    }

CLEAR_MSG;
    {
printf("dealloc_table\n");
        int rc = dealloc_table(sock, msg, module_id, table_id, &actual_module_id, &actual_table_id);
        if (rc < 0) fatal_error(rc, "dealloc_table");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_table_id != table_id) fatal_error(-1, "table mismatch: %d, %d", table_id, actual_table_id);
    }

// --

CLEAR_MSG;
    {
printf("map_ports\n");
        uint32_t table_map[] = { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14 }; // sizeof(u32) * ENCL_FW_TABLE_ENTRY_ARRAY
        int rc = map_ports(sock, msg, module_id, table_map, &actual_module_id);
        if (rc < 0) fatal_error(rc, "map_ports");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    }

// --

    uint32_t port_id = 0;
    uint32_t actual_port_id = 0;

// num_ports from get_module_info (above)
for (uint32_t port_id = 0; port_id < num_ports; port_id++) {
CLEAR_MSG;
    {
printf("get_port_state\n");
        link_state_t link_state; memset(&link_state, 0, sizeof(link_state_t));
        int rc = get_port_state(sock, msg, module_id, port_id, &actual_module_id, &actual_port_id, &link_state);
        if (rc < 0) fatal_error(rc, "get_port_state");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
    }
}

    buf_desc_t buf = {
        .len = 0,
        .frame = NULL,
    };
CLEAR_MSG;
    {
printf("send_ait_message\n");
        uint32_t message_length;
        uint8_t *frame;
        int rc = send_ait_message(sock, msg, module_id, port_id, buf, &actual_module_id, &actual_port_id);
        if (rc < 0) fatal_error(rc, "send_ait_message");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
    }

CLEAR_MSG;
    {
printf("signal_ait_message\n");
        int rc = signal_ait_message(sock, msg, module_id, port_id, buf, &actual_module_id, &actual_port_id);
        if (rc < 0) fatal_error(rc, "signal_ait_message");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
    }

    alo_reg_t alo_reg = {
        .ar_no = 0,
        .ar_data = 0,
    };
CLEAR_MSG;
    {
printf("retrieve_ait_message\n");
        buf_desc_t actual_buf; memset(&actual_buf, 0, sizeof(buf_desc_t));
        int rc = retrieve_ait_message(sock, msg, module_id, port_id, alo_reg, &actual_module_id, &actual_port_id, &actual_buf);
        if (rc < 0) fatal_error(rc, "retrieve_ait_message");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
    }

CLEAR_MSG;
    {
printf("write_alo_register\n");
        int rc = write_alo_register(sock, msg, module_id, port_id, alo_reg, &actual_module_id, &actual_port_id);
        if (rc < 0) fatal_error(rc, "write_alo_register");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
    }

CLEAR_MSG;
    {
printf("read_alo_registers\n");
        uint32_t *fp = NULL; // FIXME
        uint64_t **vp = NULL; // FIXME
        int rc = read_alo_registers(sock, msg, module_id, port_id, alo_reg, &actual_module_id, &actual_port_id, fp, vp);
        if (rc < 0) fatal_error(rc, "read_alo_registers");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
    }

// --

CLEAR_MSG;
    {
printf("send_discover_message\n");
        int rc = send_discover_message(sock, msg, module_id, port_id, buf);
        if (rc < 0) fatal_error(rc, "send_discover_message");
    }
}

// ref: lib/nl.c
// nl_send_auto_complete(sock, msg)
// nl_recvmsgs(sk, sk->s_cb);
// nl_recvmsgs_report(sk, cb);
// if (cb->cb_recvmsgs_ow) return cb->cb_recvmsgs_ow(sk, cb); else return recvmsgs(sk, cb);
// nl_recv();

int main(int argc, char *argv[]) {
    int err;

    struct nl_sock *sock = nl_socket_alloc();
    nl_connect(sock, NETLINK_GENERIC);
    // nl_socket_disable_seq_check(sock); // FIXME: resp seqno = req seqno

    // ref: lib/genl/mngt.c
    if ((err = genl_register_family(&ops)) < 0) {
        fatal_error(err, "Unable to register Generic Netlink family: \"%s\"", ops.o_name);
    }

    if ((err = genl_ops_resolve(sock, &ops)) < 0) {
        fatal_error(err, "Unable to resolve family: \"%s\"", ops.o_name);
    }

    char *nlctrl = "nlctrl";
    if (genl_ctrl_resolve(sock, nlctrl) != GENL_ID_CTRL) {
        fatal_error(NLE_INVAL, "Resolving of \"%s\" failed", nlctrl);
    }

    struct nl_msg *msg = nlmsg_alloc();
    if (msg == NULL) {
        fatal_error(NLE_NOMEM, "Unable to allocate netlink message");
    }

    doit(sock, msg);

printf("success, clean up\n");

    nlmsg_free(msg);
    nl_close(sock);
    nl_socket_free(sock);
    return 0;
}

#define NLA_DATA(na) ((void *) ((char *) (na) + NLA_HDRLEN))

static inline struct nlmsghdr *genlmsg_nlhdr(void *user_hdr) {
    return (struct nlmsghdr *) ((char *) user_hdr - GENL_HDRLEN - NLMSG_HDRLEN);
}

#define PRINTIT printf
static void dump_msg(void *user_hdr) {
    struct nlmsghdr *nh = genlmsg_nlhdr(user_hdr);
    struct genlmsghdr *gh = nlmsg_data(nh);
    struct nlattr *head = (struct nlattr *) user_hdr; // GENLMSG_DATA(nh);
    void *after = (void *) (((char *) nh) + nh->nlmsg_len);

    PRINTIT("    nh: %p\n", nh);
    PRINTIT("    .nlmsg_len: %d\n", nh->nlmsg_len);
    PRINTIT("    .nlmsg_type: %d\n", nh->nlmsg_type);
    PRINTIT("    .nlmsg_flags: %d\n", nh->nlmsg_flags);
    PRINTIT("    .nlmsg_seq: %d\n", nh->nlmsg_seq);
    PRINTIT("    .nlmsg_pid: %d\n", nh->nlmsg_pid);

    PRINTIT("    gh: %p\n", gh);
    PRINTIT("    .cmd: %d\n", gh->cmd);
    PRINTIT("    .version: %d\n", gh->version);
    PRINTIT("    .reserved: %d\n", gh->reserved);

    PRINTIT("\n");
    PRINTIT("    after: %p\n", after);
    PRINTIT("    payload: %p\n", head);

    // trusts that there's enough space for nla hdr and data
    // FIXME: after could compare against NLA_HDRLEN, then against NLMSG_ALIGN(p->nla_len)
    for (struct nlattr *p = head; after > (void *) p; p = (struct nlattr *) (((char *) p) + NLMSG_ALIGN(p->nla_len))) {
        int nbytes = p->nla_len - NLA_HDRLEN; // ((int) NLA_ALIGN(sizeof(struct nlattr)))
        void *d = NLA_DATA(p);
        PRINTIT("    nla: %p .nla_type: %d .nla_len: %d .data: %p nbytes: %d align: %d\n", p, p->nla_type, p->nla_len, d, nbytes, NLMSG_ALIGN(p->nla_len));
        PRINTIT("      ");
        // flag: nbytes == 0
        if (nbytes == 1) { PRINTIT("%d (%02x)\n", *(char *) d, *(char *) d); continue; }
        if (nbytes == 2) { PRINTIT("%d (%04x)\n", *(short *) d, *(short *) d); continue; }
        if (nbytes == 4) { PRINTIT("%d (%08x)\n", *(int *) d, *(int *) d); continue; }
        if (nbytes == 8) { PRINTIT("%ld (%016lx)\n", *(long *) d, *(long *) d); continue; }

        for (int i = 0; i < nbytes; i++) {
            PRINTIT("%02x", ((char *) d)[i]);
            if (i % 16 == 15) PRINTIT("\n        ");
        }
        PRINTIT("\n");
    }
}

// be paranoid, probably parses with excess ';'
#define NLAPUT_CHECKED(putattr) { int rc = putattr; if (rc) return rc; }

// FIXME: string leak
static int get_link_state(struct nlattr **tb, link_state_t *lp) {
    char *module_name = nla_strdup(tb[NL_ECNL_ATTR_MODULE_NAME]); // nla_get_string
    char *port_name = nla_strdup(tb[NL_ECNL_ATTR_PORT_NAME]); // nla_get_string
    uint32_t port_link_state = nla_get_u32(tb[NL_ECNL_ATTR_PORT_LINK_STATE]);
    uint64_t port_s_counter = nla_get_u64(tb[NL_ECNL_ATTR_PORT_S_COUNTER]);
    uint64_t port_r_counter = nla_get_u64(tb[NL_ECNL_ATTR_PORT_R_COUNTER]);
    uint64_t port_recover_counter = nla_get_u64(tb[NL_ECNL_ATTR_PORT_RECOVER_COUNTER]);
    uint64_t port_recovered_counter = nla_get_u64(tb[NL_ECNL_ATTR_PORT_RECOVERED_COUNTER]);
    uint64_t port_entt_count = nla_get_u64(tb[NL_ECNL_ATTR_PORT_ENTT_COUNT]);
    uint64_t port_aop_count = nla_get_u64(tb[NL_ECNL_ATTR_PORT_AOP_COUNT]);
    uint32_t num_ait_messages = nla_get_u32(tb[NL_ECNL_ATTR_NUM_AIT_MESSAGES]);

    lp->module_name = module_name; // FIXME: leak
    lp->port_name = port_name; // FIXME: leak
    lp->port_link_state = port_link_state;
    lp->port_s_counter = port_s_counter;
    lp->port_r_counter = port_r_counter;
    lp->port_recover_counter = port_recover_counter;
    lp->port_recovered_counter = port_recovered_counter;
    lp->port_entt_count = port_entt_count;
    lp->port_aop_count = port_aop_count;
    lp->num_ait_messages = num_ait_messages;
    return 0;
}

// "ecnl0"
// GET_MODULE_INFO(uint32_t module_id)
static int get_module_info(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, module_info_t *mip) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_GET_MODULE_INFO, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }
{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    char *module_name = nla_strdup(cbi.tb[NL_ECNL_ATTR_MODULE_NAME]); // nla_get_string
    uint32_t num_ports = nla_get_u32(cbi.tb[NL_ECNL_ATTR_NUM_PORTS]);
    if (mip != NULL) {
        mip->module_id = module_id;
        mip->module_name = module_name; // FIXME leak
        mip->num_ports = num_ports;
    }
}
WAIT_ACK;
    return 0;
}

// GET_PORT_STATE(uint32_t module_id, uint32_t port_id)
static int get_port_state(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, uint32_t *mp, uint32_t *pp, link_state_t *lp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_GET_PORT_STATE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_PORT_ID, port_id));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_PORT_ID]);
    link_state_t ls; memset(&ls, 0, sizeof(link_state_t)); get_link_state(cbi.tb, &ls);
    *mp = module_id;
    *pp = port_id;
#if 0
    *lp = ls; // FIXME: copy?
#endif
}
WAIT_ACK;
    return 0;
}

// --

// ALLOC_DRIVER(char *module_name)
static int alloc_driver(struct nl_sock *sock, struct nl_msg *msg, char *module_name, uint32_t *mp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_ALLOC_DRIVER, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_string(msg, NL_ECNL_ATTR_MODULE_NAME, module_name));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    *mp = module_id;
}
WAIT_ACK;
    return 0;
}

// ALLOC_TABLE(uint32_t module_id, uint32_t table_size)
static int alloc_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_size, uint32_t *mp, uint32_t *tp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_ALLOC_TABLE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_SIZE, table_size));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t table_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_TABLE_ID]);
    *mp = module_id;
    *tp = table_id;
}
WAIT_ACK;
    return 0;
}

// DEALLOC_TABLE(uint32_t module_id, uint32_t table_id)
static int dealloc_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t *mp, uint32_t *tp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_DEALLOC_TABLE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_ID, table_id));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t table_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_TABLE_ID]);
    *mp = module_id;
    *tp = table_id;
}
WAIT_ACK;
    return 0;
}

// SELECT_TABLE(uint32_t module_id, uint32_t table_id)
static int select_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t *mp, uint32_t *tp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_SELECT_TABLE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_ID, table_id));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t table_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_TABLE_ID]);
    *mp = module_id;
    *tp = table_id;
}
WAIT_ACK;
    return 0;
}

// FILL_TABLE(uint32_t module_id, uint32_t table_id, uint32_t table_location, uint32_t table_content_size, ecnl_table_entry_t *table_content)
static int fill_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t table_location, uint32_t table_content_size, ecnl_table_entry_t *table_content, uint32_t *mp, uint32_t *tp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_FILL_TABLE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_ID, table_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_LOCATION, table_location));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_CONTENT_SIZE, table_content_size));
    NLAPUT_CHECKED(nla_put(msg, NL_ECNL_ATTR_TABLE_CONTENT, sizeof(ecnl_table_entry_t) * table_content_size, table_content));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t table_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_TABLE_ID]);
    *mp = module_id;
    *tp = table_id;
}
WAIT_ACK;
    return 0;
}

// FILL_TABLE_ENTRY(uint32_t module_id, uint32_t table_id, uint32_t table_location, ecnl_table_entry_t *table_entry)
static int fill_table_entry(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t table_location, ecnl_table_entry_t *table_entry, uint32_t *mp, uint32_t *tp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_FILL_TABLE_ENTRY, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_ID, table_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_ENTRY_LOCATION, table_location));
    NLAPUT_CHECKED(nla_put(msg, NL_ECNL_ATTR_TABLE_ENTRY, sizeof(ecnl_table_entry_t), table_entry));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t table_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_TABLE_ID]);
    *mp = module_id;
    *tp = table_id;
}
WAIT_ACK;
    return 0;
}

// MAP_PORTS(uint32_t module_id, uint32_t *table_map)
static int map_ports(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t *table_map, uint32_t *mp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_MAP_PORTS, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put(msg, NL_ECNL_ATTR_TABLE_MAP, sizeof(uint32_t) * ENCL_FW_TABLE_ENTRY_ARRAY, table_map));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    *mp = module_id;
}
WAIT_ACK;
    return 0;
}

// START_FORWARDING(uint32_t module_id)
static int start_forwarding(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t *mp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_START_FORWARDING, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    *mp = module_id;
}
WAIT_ACK;
    return 0;
}

// STOP_FORWARDING(uint32_t module_id)
static int stop_forwarding(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t *mp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_STOP_FORWARDING, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    *mp = module_id;
}
WAIT_ACK;
    return 0;
}

// READ_ALO_REGISTERS(uint32_t module_id, uint32_t port_id, uint64_t alo_reg_data, uint32_t alo_reg_no)
static int read_alo_registers(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp, uint32_t *fp, uint64_t **vp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_READ_ALO_REGISTERS, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_PORT_ID, port_id));
    NLAPUT_CHECKED(nla_put_u64(msg, NL_ECNL_ATTR_ALO_REG_DATA, alo_reg.ar_data));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_ALO_REG_NO, alo_reg.ar_no));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_PORT_ID]);
    uint32_t alo_flag = nla_get_u32(cbi.tb[NL_ECNL_ATTR_ALO_FLAG]);
    int nbytes = nla_len(cbi.tb[NL_ECNL_ATTR_ALO_REG_VALUES]);
    uint64_t p[32]; nla_memcpy(p, cbi.tb[NL_ECNL_ATTR_ALO_REG_VALUES], sizeof(uint64_t) * 32); // nla_get_unspec
    *mp = module_id;
    *pp = port_id;
#if 0
    *fp = alo_flag;
    *vp = p; // FIXME
#endif
}
WAIT_ACK;
    return 0;
}

// RETRIEVE_AIT_MESSAGE(uint32_t module_id, uint32_t port_id, uint64_t alo_reg_data, uint32_t alo_reg_no)
static int retrieve_ait_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp, buf_desc_t *buf) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_RETRIEVE_AIT_MESSAGE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_PORT_ID, port_id));
    // FIXME: suspect ?
    NLAPUT_CHECKED(nla_put_u64(msg, NL_ECNL_ATTR_ALO_REG_DATA, alo_reg.ar_data));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_ALO_REG_NO, alo_reg.ar_no));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_PORT_ID]);
    uint32_t message_length = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MESSAGE_LENGTH]);
    int nbytes = nla_len(cbi.tb[NL_ECNL_ATTR_MESSAGE]);
    uint8_t p[4096]; memset(p, 0, sizeof(p)); nla_memcpy(p, cbi.tb[NL_ECNL_ATTR_MESSAGE], message_length); // nla_get_unspec
    *mp = module_id;
    *pp = port_id;
    buf->len = message_length;
    buf->frame = NULL;
#if 0
    *buf->frame = p; // FIXME
#endif
}
WAIT_ACK;
    return 0;
}

// WRITE_ALO_REGISTER(uint32_t module_id, uint32_t port_id, uint64_t alo_reg_data, uint32_t alo_reg_no)
static int write_alo_register(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_WRITE_ALO_REGISTER, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_PORT_ID, port_id));
    NLAPUT_CHECKED(nla_put_u64(msg, NL_ECNL_ATTR_ALO_REG_DATA, alo_reg.ar_data));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_ALO_REG_NO, alo_reg.ar_no));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_PORT_ID]);
    *mp = module_id;
    *pp = port_id;
}
WAIT_ACK;
    return 0;
}

// SEND_AIT_MESSAGE(uint32_t module_id, uint32_t port_id, uint32_t message_length, uint8_t *frame)
static int send_ait_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, buf_desc_t buf, uint32_t *mp, uint32_t *pp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_SEND_AIT_MESSAGE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_PORT_ID, port_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MESSAGE_LENGTH, buf.len));
    NLAPUT_CHECKED(nla_put(msg, NL_ECNL_ATTR_MESSAGE, buf.len, buf.frame));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_PORT_ID]);
    *mp = module_id;
    *pp = port_id;
}
WAIT_ACK;
    return 0;
}

// asynchronous publish (i.e. pub-sub events)

// RETRIEVE_AIT_MESSAGE, DISCOVERY
static int event_receive_dsc(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint8_t *dp) {
    uint32_t module_id = nla_get_u32(tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(tb[NL_ECNL_ATTR_PORT_ID]);
int len = 0; // FIXME
    int nbytes = nla_len(tb[NL_ECNL_ATTR_DISCOVERING_MSG]);
    uint8_t p[4096]; memset(p, 0, sizeof(p)); nla_memcpy(p, tb[NL_ECNL_ATTR_DISCOVERING_MSG], len); // nla_get_unspec
    *mp = module_id;
    *pp = port_id;
#if 0
    *dp = p; // FIXME
#endif
    return 0;
}

// GET_PORT_STATE, LINKSTATUS
static int event_link_status_update(struct nlattr **tb, uint32_t *mp, uint32_t *pp, link_state_t *lp) {
    uint32_t module_id = nla_get_u32(tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(tb[NL_ECNL_ATTR_PORT_ID]);
    link_state_t ls; memset(&ls, 0, sizeof(link_state_t)); get_link_state(tb, &ls); // FIXME: trash
    *mp = module_id;
    *pp = port_id;
#if 0
    *lp = ls; // FIXME: copy?
#endif
    return 0;
}

// RETRIEVE_AIT_MESSAGE, AIT
static int event_forward_ait_message(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint32_t *lp, uint8_t *dp) {
    uint32_t module_id = nla_get_u32(tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(tb[NL_ECNL_ATTR_PORT_ID]);
    uint32_t message_length = nla_get_u32(tb[NL_ECNL_ATTR_MESSAGE_LENGTH]);
    int nbytes = nla_len(tb[NL_ECNL_ATTR_MESSAGE]);
    uint8_t p[4096]; memset(p, 0, sizeof(p)); nla_memcpy(p, tb[NL_ECNL_ATTR_MESSAGE], message_length); // nla_get_unspec
    *mp = module_id;
    *pp = port_id;
#if 0
    *lp = message_length;
    *dp = p; // FIXME
#endif
    return 0;
}

// SIGNAL_AIT_MESSAGE, AIT
static int event_got_ait_massage(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint32_t *lp) {
    uint32_t module_id = nla_get_u32(tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(tb[NL_ECNL_ATTR_PORT_ID]);
    uint32_t num_ait_messages = nla_get_u32(tb[NL_ECNL_ATTR_NUM_AIT_MESSAGES]);
    *mp = module_id;
    *pp = port_id;
#if 0
    *lp = num_ait_messages;
#endif
    return 0;
}

// READ_ALO_REGISTERS, AIT
static int event_got_alo_update(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint64_t *vp, uint32_t *fp) {
    uint32_t module_id = nla_get_u32(tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(tb[NL_ECNL_ATTR_PORT_ID]);
    int nbytes = nla_len(tb[NL_ECNL_ATTR_ALO_REG_VALUES]);
    uint8_t p[4096]; memset(p, 0, sizeof(p)); nla_memcpy(p, tb[NL_ECNL_ATTR_ALO_REG_VALUES], sizeof(uint64_t) * 32); // nla_get_unspec
    uint32_t alo_flag = nla_get_u32(tb[NL_ECNL_ATTR_ALO_FLAG]);
    *mp = module_id;
    *pp = port_id;
#if 0
    *vp = p; // FIXME
    *fp = alo_flag;
#endif
    return 0;
}

// --

// fire-and-forget (i.e. no response)

// SEND_DISCOVER_MESSAGE(uint32_t module_id, uint32_t port_id, uint32_t message_length, uint8_t *message)
static int send_discover_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, buf_desc_t buf) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_SEND_DISCOVER_MESSAGE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_PORT_ID, port_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MESSAGE_LENGTH, buf.len));
    NLAPUT_CHECKED(nla_put(msg, NL_ECNL_ATTR_MESSAGE, buf.len, buf.frame));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }
    return 0;
}

// --

// dummy func ?? (aka send_ait_message)

// SIGNAL_AIT_MESSAGE(uint32_t module_id, uint32_t port_id, uint32_t message_length, uint8_t *message)
static int signal_ait_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, buf_desc_t buf, uint32_t *mp, uint32_t *pp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_SIGNAL_AIT_MESSAGE, ECNL_GENL_VERSION);
    // ref: send_ait_message()
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_PORT_ID, port_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MESSAGE_LENGTH, buf.len));
    NLAPUT_CHECKED(nla_put(msg, NL_ECNL_ATTR_MESSAGE, buf.len, buf.frame));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "Unable to receive message: %s", nl_geterror(err)); }
    printf("nl_recvmsgs_default: %d msgs processed\n\n", err);
    uint32_t module_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(cbi.tb[NL_ECNL_ATTR_PORT_ID]);
    *mp = module_id;
    *pp = port_id;
}
WAIT_ACK;
    return 0;
}

