#include "ecnl_proto.h"

int ecp_verbose = 1;
#define ECP_DEBUG(fmt, args...) if (ecp_verbose) { printf(fmt, ## args); } else { }
#define FAM_DEBUG(fmt, args...) if (ecp_verbose) { printf(ECNL_GENL_NAME ": " fmt, ## args); } else { }

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
extern void fatal_error(int err, const char *fmt, ...) {
    va_list ap;
    fprintf(stderr, "Error: %s - ", strerror(err));
    va_start(ap, fmt);
    vfprintf(stderr, fmt, ap);
    va_end(ap);
    fprintf(stderr, "\n");
    exit(abs(err));
}

static void dump_block(void *d, int nbytes) {
    ECP_DEBUG("nbytes: %d\n        ", nbytes);
    for (int i = 0; i < nbytes; i++) {
        ECP_DEBUG("%02x", ((char *) d)[i] & 0xff);
        if (i % 16 == 15) ECP_DEBUG("\n        ");
    }
    ECP_DEBUG("\n");
}

static void copy_unspec(callback_index_t *cbi, uint64_t attr) {
    int nbytes = nla_len(cbi->tb[attr]);
    uint8_t *p = malloc(nbytes); if (!p) { perror("malloc"); return; } // memset(p, 0, nbytes);
    nla_memcpy(p, cbi->tb[attr], nbytes); // nla_get_unspec

    if (attr == NL_ECNL_ATTR_MESSAGE) {
        cbi->msg = p;
        cbi->msg_bytes = nbytes;
    }

    if (attr == NL_ECNL_ATTR_DISCOVERING_MSG) {
        cbi->disc_msg = p;
        cbi->disc_msg_bytes = nbytes;
    }
}

// nla_len nla_data nla_get_string
// nla_get nla_put nla_memcpy
static void grab_attr(callback_index_t *cbi, uint64_t attr) {
    switch (attr) {
    case NL_ECNL_ATTR_ALO_FLAG:               cbi->alo_flag =               nla_get_u32(cbi->tb[NL_ECNL_ATTR_ALO_FLAG]); break;
    case NL_ECNL_ATTR_MESSAGE_LENGTH:         cbi->message_length =         nla_get_u32(cbi->tb[NL_ECNL_ATTR_MESSAGE_LENGTH]); break;
    case NL_ECNL_ATTR_MODULE_ID:              cbi->module_id =              nla_get_u32(cbi->tb[NL_ECNL_ATTR_MODULE_ID]); break;
    case NL_ECNL_ATTR_NUM_AIT_MESSAGES:       cbi->num_ait_messages =       nla_get_u32(cbi->tb[NL_ECNL_ATTR_NUM_AIT_MESSAGES]); break;
    case NL_ECNL_ATTR_NUM_PORTS:              cbi->num_ports =              nla_get_u32(cbi->tb[NL_ECNL_ATTR_NUM_PORTS]); break;
    case NL_ECNL_ATTR_PORT_ID:                cbi->port_id =                nla_get_u32(cbi->tb[NL_ECNL_ATTR_PORT_ID]); break;
    case NL_ECNL_ATTR_PORT_LINK_STATE:        cbi->port_link_state =        nla_get_u32(cbi->tb[NL_ECNL_ATTR_PORT_LINK_STATE]); break;
    case NL_ECNL_ATTR_TABLE_ID:               cbi->table_id =               nla_get_u32(cbi->tb[NL_ECNL_ATTR_TABLE_ID]); break;

    case NL_ECNL_ATTR_PORT_AOP_COUNT:         cbi->port_aop_count =         nla_get_u64(cbi->tb[NL_ECNL_ATTR_PORT_AOP_COUNT]); break;
    case NL_ECNL_ATTR_PORT_ENTT_COUNT:        cbi->port_entt_count =        nla_get_u64(cbi->tb[NL_ECNL_ATTR_PORT_ENTT_COUNT]); break;
    case NL_ECNL_ATTR_PORT_R_COUNTER:         cbi->port_r_counter =         nla_get_u64(cbi->tb[NL_ECNL_ATTR_PORT_R_COUNTER]); break;
    case NL_ECNL_ATTR_PORT_RECOVER_COUNTER:   cbi->port_recover_counter =   nla_get_u64(cbi->tb[NL_ECNL_ATTR_PORT_RECOVER_COUNTER]); break;
    case NL_ECNL_ATTR_PORT_RECOVERED_COUNTER: cbi->port_recovered_counter = nla_get_u64(cbi->tb[NL_ECNL_ATTR_PORT_RECOVERED_COUNTER]); break;
    case NL_ECNL_ATTR_PORT_S_COUNTER:         cbi->port_s_counter =         nla_get_u64(cbi->tb[NL_ECNL_ATTR_PORT_S_COUNTER]); break;

    case NL_ECNL_ATTR_MODULE_NAME:            cbi->module_name =            strdup(nla_get_string(cbi->tb[NL_ECNL_ATTR_MODULE_NAME])); break; // potential leak
    case NL_ECNL_ATTR_PORT_NAME:              cbi->port_name =              strdup(nla_get_string(cbi->tb[NL_ECNL_ATTR_PORT_NAME]));   break; // potential leak

    case NL_ECNL_ATTR_ALO_REG_VALUES: nla_memcpy(cbi->regblk, cbi->tb[NL_ECNL_ATTR_ALO_REG_VALUES], ALO_REGBLK_SIZE); break; // nla_get_unspec

    case NL_ECNL_ATTR_MESSAGE:                copy_unspec(cbi, NL_ECNL_ATTR_MESSAGE);         break; // potential leak
    case NL_ECNL_ATTR_DISCOVERING_MSG:        copy_unspec(cbi, NL_ECNL_ATTR_DISCOVERING_MSG); break; // potential leak
    }
}

// .genlhdr
// .userhdr
// nl_cb_action - NL_OK, NL_SKIP, NL_STOP
static int parse_generic(struct nl_cache_ops *unused, struct genl_cmd *cmd, struct genl_info *info, void *arg) {
    callback_index_t *cbi = (callback_index_t *) arg;
    if (cbi->magic != 0x5a5a) fatal_error(-1, "garbled callback arg");
    struct nlmsghdr *nlh = info->nlh;

    ECP_DEBUG("parse_generic:\n");

    if (nlh->nlmsg_type == NLMSG_ERROR) { ECP_DEBUG("NLMSG_ERROR\n"); return -1; } // FIXME: should this be NL_OK ??

    int err = genlmsg_parse(nlh, 0, &cbi->tb[0], NL_ECNL_ATTR_MAX, attr_policy);
    if (err < 0) { ECP_DEBUG("genlmsg_parse error\n"); return NL_SKIP; } // FIXME: what return code here ??

    for (int i = 0; i < ARRAY_SIZE(attr_names); i++) {
        const trans_tbl_t *tp = &attr_names[i];
        uint64_t attr = tp->i;
        struct nlattr *na = cbi->tb[attr];
        if (na == NULL) continue;

        grab_attr(cbi, attr); // module_name, port_name, msg, disc_msg -  need to be free'ed by client

        struct nla_policy *pp = &attr_policy[attr];
        switch (pp->type) {
        // NLA_FLAG NLA_U8 NLA_U16 NLA_MSECS NLA_STRING NLA_UNSPEC NLA_NESTED
        case NLA_U32: ECP_DEBUG("%s(%ld): %d\n", tp->a, attr, nla_get_u32(na)); continue;
        case NLA_U64: ECP_DEBUG("%s(%ld): %ld\n", tp->a, attr, nla_get_u64(na)); continue;
        case NLA_NUL_STRING: ECP_DEBUG("%s(%ld): \"%s\"\n", tp->a, attr, nla_get_string(na)); continue;
        case NL_ECNL_ATTR_UNSPEC: ECP_DEBUG("%s(%ld): block ", tp->a, attr); dump_block(nla_data(na), nla_len(na)); continue;
        }
        ECP_DEBUG("%s (%ld)\n", tp->a, attr);
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
        ECP_DEBUG("%s pid %u uid %u gid %u parent %u\n", yy->ac_comm, yy->ac_pid, yy->ac_uid, yy->ac_gid, yy->ac_ppid);
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

#define NLA_DATA(na) ((void *) ((char *) (na) + NLA_HDRLEN))

static inline struct nlmsghdr *genlmsg_nlhdr(void *user_hdr) {
    return (struct nlmsghdr *) ((char *) user_hdr - GENL_HDRLEN - NLMSG_HDRLEN);
}

// --

#define PRINTIT printf
extern void dump_msg(void *user_hdr) {
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
extern int get_link_state(callback_index_t *cbi, link_state_t *lp) {
    lp->module_name =            cbi->module_name; // FIXME: leak
    lp->port_name =              cbi->port_name; // FIXME: leak
    lp->port_link_state =        cbi->port_link_state;
    lp->port_s_counter =         cbi->port_s_counter;
    lp->port_r_counter =         cbi->port_r_counter;
    lp->port_recover_counter =   cbi->port_recover_counter;
    lp->port_recovered_counter = cbi->port_recovered_counter;
    lp->port_entt_count =        cbi->port_entt_count;
    lp->port_aop_count =         cbi->port_aop_count;
    lp->num_ait_messages =       cbi->num_ait_messages;
    return 0;
}

#define ANALYZE_REPLY(fmt, args...) \
    callback_index_t cbi = { .magic = 0x5a5a }; \
    struct nl_cb *s_cb = nl_socket_get_cb(sock); \
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "Unable to modify valid message callback"); } \
    if ((err = nl_recvmsgs_report(sock, s_cb)) < 0) { fatal_error(err, fmt "Unable to receive message: %s", ##args, nl_geterror(err)); } \
    ECP_DEBUG("nl_recvmsgs_report: %d msgs processed\n\n", err);

// "ecnl0"
// GET_MODULE_INFO(uint32_t module_id)
extern int get_module_info(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, module_info_t *mip) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_GET_MODULE_INFO, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }
{
    ANALYZE_REPLY("get_module_info(\"%s\", %d) : ", ops.o_name, ops.o_id);
    if (mip != NULL) {
        mip->module_id = cbi.module_id;
        mip->module_name = cbi.module_name; // FIXME leak
        mip->num_ports = cbi.num_ports;
    }
}
WAIT_ACK;
    return 0;
}

// GET_PORT_STATE(uint32_t module_id, uint32_t port_id)
extern int get_port_state(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, uint32_t *mp, uint32_t *pp, link_state_t *lp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_GET_PORT_STATE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_PORT_ID, port_id));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    ANALYZE_REPLY("get_port_state");
    *mp = cbi.module_id;
    *pp = cbi.port_id;
    get_link_state(&cbi, lp);
}
WAIT_ACK;
    return 0;
}

// --

// ALLOC_DRIVER(char *module_name)
extern int alloc_driver(struct nl_sock *sock, struct nl_msg *msg, char *module_name, uint32_t *mp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_ALLOC_DRIVER, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_string(msg, NL_ECNL_ATTR_MODULE_NAME, module_name));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    ANALYZE_REPLY("alloc_driver");
    *mp = cbi.module_id;
}
WAIT_ACK;
    return 0;
}

// ALLOC_TABLE(uint32_t module_id, uint32_t table_size)
extern int alloc_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_size, uint32_t *mp, uint32_t *tp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_ALLOC_TABLE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_SIZE, table_size));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    ANALYZE_REPLY("alloc_table");
    *mp = cbi.module_id;
    *tp = cbi.table_id;
}
WAIT_ACK;
    return 0;
}

// DEALLOC_TABLE(uint32_t module_id, uint32_t table_id)
extern int dealloc_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t *mp, uint32_t *tp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_DEALLOC_TABLE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_ID, table_id));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    ANALYZE_REPLY("dealloc_table");
    *mp = cbi.module_id;
    *tp = cbi.table_id;
}
WAIT_ACK;
    return 0;
}

// SELECT_TABLE(uint32_t module_id, uint32_t table_id)
extern int select_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t *mp, uint32_t *tp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_SELECT_TABLE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_ID, table_id));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    ANALYZE_REPLY("select_table");
    *mp = cbi.module_id;
    *tp = cbi.table_id;
}
WAIT_ACK;
    return 0;
}

// FILL_TABLE(uint32_t module_id, uint32_t table_id, uint32_t table_location, uint32_t table_content_size, ecnl_table_entry_t *table_content)
extern int fill_table(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t table_location, uint32_t table_content_size, ecnl_table_entry_t *table_content, uint32_t *mp, uint32_t *tp) {
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
    ANALYZE_REPLY("fill_table");
    *mp = cbi.module_id;
    *tp = cbi.table_id;
}
WAIT_ACK;
    return 0;
}

// FILL_TABLE_ENTRY(uint32_t module_id, uint32_t table_id, uint32_t table_location, ecnl_table_entry_t *table_entry)
extern int fill_table_entry(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t table_id, uint32_t table_location, ecnl_table_entry_t *table_entry, uint32_t *mp, uint32_t *tp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_FILL_TABLE_ENTRY, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_ID, table_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_TABLE_ENTRY_LOCATION, table_location));
    NLAPUT_CHECKED(nla_put(msg, NL_ECNL_ATTR_TABLE_ENTRY, sizeof(ecnl_table_entry_t), table_entry));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    ANALYZE_REPLY("fill_table_entry");
    *mp = cbi.module_id;
    *tp = cbi.table_id;
}
WAIT_ACK;
    return 0;
}

// MAP_PORTS(uint32_t module_id, uint32_t *table_map)
extern int map_ports(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t *table_map, uint32_t *mp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_MAP_PORTS, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put(msg, NL_ECNL_ATTR_TABLE_MAP, sizeof(uint32_t) * ENCL_FW_TABLE_ENTRY_ARRAY, table_map));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    ANALYZE_REPLY("map_ports");
    *mp = cbi.module_id;
}
WAIT_ACK;
    return 0;
}

// START_FORWARDING(uint32_t module_id)
extern int start_forwarding(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t *mp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_START_FORWARDING, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    ANALYZE_REPLY("start_forwarding");
    *mp = cbi.module_id;
}
WAIT_ACK;
    return 0;
}

// STOP_FORWARDING(uint32_t module_id)
extern int stop_forwarding(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t *mp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_STOP_FORWARDING, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    ANALYZE_REPLY("stop_forwarding");
    *mp = cbi.module_id;
}
WAIT_ACK;
    return 0;
}

// READ_ALO_REGISTERS(uint32_t module_id, uint32_t port_id, uint64_t alo_reg_data, uint32_t alo_reg_no)
extern int read_alo_registers(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp, uint32_t *fp, uint64_t **vp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_READ_ALO_REGISTERS, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_PORT_ID, port_id));
    NLAPUT_CHECKED(nla_put_u64(msg, NL_ECNL_ATTR_ALO_REG_DATA, alo_reg.ar_data));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_ALO_REG_NO, alo_reg.ar_no));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    ANALYZE_REPLY("read_alo_registers");
    *mp = cbi.module_id;
    *pp = cbi.port_id;
#if 0
    *fp = cbi.alo_flag;
    *vp = cbi.p; // FIXME
    *regblk = cbi.regblk;
#endif
}
WAIT_ACK;
    return 0;
}

// RETRIEVE_AIT_MESSAGE(uint32_t module_id, uint32_t port_id, uint64_t alo_reg_data, uint32_t alo_reg_no)
extern int retrieve_ait_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp, buf_desc_t *buf) {
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
    ANALYZE_REPLY("retrieve_ait_message");

    *mp = cbi.module_id;
    *pp = cbi.port_id;

    if (!buf) {
        ECP_DEBUG("retrieve_ait_message - no result buffer ?\n");
        return 0;
    }

    uint32_t message_length = cbi.message_length;
    uint8_t *msg = cbi.msg;
    size_t msg_bytes = cbi.msg_bytes;

    if (!msg) {
        ECP_DEBUG("retrieve_ait_message - no msg?\n");
        return 0;
    }

    // 3 factors: buf->len, message_length, msg_bytes
    if (message_length != msg_bytes) {
        ECP_DEBUG("retrieve_ait_message - WARN: message_length (%d) != msg_bytes (%lu)\n", message_length, msg_bytes);
        if (msg_bytes < message_length) message_length = msg_bytes;
    }

    if (!buf->frame) {
        ECP_DEBUG("retrieve_ait_message - allocating return buffer (%d)\n", message_length);
        buf->frame = msg; // potential leak : client responsiblity
        buf->len = message_length;
    }
    else if (buf->len < message_length) {
        ECP_DEBUG("retrieve_ait_message - return buffer too small (%d), reallocated (%d)\n", buf->len, message_length);
        buf->frame = msg; // definite leak : FIXME ??
        buf->len = message_length;
    }
    else {
        memcpy(buf->frame, msg, (size_t) message_length);
        buf->len = message_length;
        free(cbi.msg); // cbi.msg_bytes
    }

    ECP_DEBUG("retr buffer: ");
    dump_block(buf->frame, buf->len);
}
WAIT_ACK;
    return 0;
}

// WRITE_ALO_REGISTER(uint32_t module_id, uint32_t port_id, uint64_t alo_reg_data, uint32_t alo_reg_no)
extern int write_alo_register(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_WRITE_ALO_REGISTER, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_PORT_ID, port_id));
    NLAPUT_CHECKED(nla_put_u64(msg, NL_ECNL_ATTR_ALO_REG_DATA, alo_reg.ar_data));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_ALO_REG_NO, alo_reg.ar_no));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

{
    ANALYZE_REPLY("write_alo_register");
    *mp = cbi.module_id;
    *pp = cbi.port_id;
}
WAIT_ACK;
    return 0;
}

// SEND_AIT_MESSAGE(uint32_t module_id, uint32_t port_id, uint32_t message_length, uint8_t *frame)
extern int send_ait_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, buf_desc_t buf, uint32_t *mp, uint32_t *pp) {
    int err;
    void *user_hdr = genlmsg_put(msg, NL_AUTO_PORT, NL_AUTO_SEQ, ops.o_id, 0, 0, NL_ECNL_CMD_SEND_AIT_MESSAGE, ECNL_GENL_VERSION);
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_PORT_ID, port_id));
    NLAPUT_CHECKED(nla_put_u32(msg, NL_ECNL_ATTR_MESSAGE_LENGTH, buf.len));
    NLAPUT_CHECKED(nla_put(msg, NL_ECNL_ATTR_MESSAGE, buf.len, buf.frame));
    nl_complete_msg(sock, msg);
    if ((err = nl_send(sock, msg)) < 0) { fatal_error(err, "Unable to send message: %s", nl_geterror(err)); }

    ECP_DEBUG("send buffer: ");
    dump_block(buf.frame, buf.len);

{
    ANALYZE_REPLY("send_ait_message");
    *mp = cbi.module_id;
    *pp = cbi.port_id;
}
WAIT_ACK;
    return 0;
}

// asynchronous publish (i.e. pub-sub events)

// RETRIEVE_AIT_MESSAGE, DISCOVERY
extern int event_receive_dsc(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint8_t *dp) {
    uint32_t module_id = nla_get_u32(tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(tb[NL_ECNL_ATTR_PORT_ID]);
    int nbytes = nla_len(tb[NL_ECNL_ATTR_DISCOVERING_MSG]);
    uint8_t p[4096]; memset(p, 0, sizeof(p)); nla_memcpy(p, tb[NL_ECNL_ATTR_DISCOVERING_MSG], nbytes); // nla_get_unspec
    *mp = module_id;
    *pp = port_id;
#if 0
    *dp = p; // FIXME
#endif
    return 0;
}

// GET_PORT_STATE, LINKSTATUS
extern int event_link_status_update(struct nlattr **tb, uint32_t *mp, uint32_t *pp, link_state_t *lp) {
    uint32_t module_id = nla_get_u32(tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(tb[NL_ECNL_ATTR_PORT_ID]);
    link_state_t ls; memset(&ls, 0, sizeof(link_state_t)); // get_link_state(&cbi, &ls);
    *mp = module_id;
    *pp = port_id;
#if 0
    *lp = ls; // FIXME: copy?
#endif
    return 0;
}

// RETRIEVE_AIT_MESSAGE, AIT
extern int event_forward_ait_message(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint32_t *lp, uint8_t *dp) {
    uint32_t module_id = nla_get_u32(tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(tb[NL_ECNL_ATTR_PORT_ID]);
    uint32_t message_length = nla_get_u32(tb[NL_ECNL_ATTR_MESSAGE_LENGTH]);
    // int nbytes = nla_len(tb[NL_ECNL_ATTR_MESSAGE]);
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
extern int event_got_ait_massage(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint32_t *lp) {
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
extern int event_got_alo_update(struct nlattr **tb, uint32_t *mp, uint32_t *pp, uint64_t *vp, uint32_t *fp) {
    uint32_t module_id = nla_get_u32(tb[NL_ECNL_ATTR_MODULE_ID]);
    uint32_t port_id = nla_get_u32(tb[NL_ECNL_ATTR_PORT_ID]);
    uint64_t regblk[32]; memset(regblk, 0, ALO_REGBLK_SIZE); nla_memcpy(regblk, tb[NL_ECNL_ATTR_ALO_REG_VALUES], ALO_REGBLK_SIZE); // nla_get_unspec
    uint32_t alo_flag = nla_get_u32(tb[NL_ECNL_ATTR_ALO_FLAG]);
    *mp = module_id;
    *pp = port_id;
#if 0
    *vp = regblk;
    *fp = alo_flag;
#endif
    return 0;
}

// --

// fire-and-forget (i.e. no response)

// SEND_DISCOVER_MESSAGE(uint32_t module_id, uint32_t port_id, uint32_t message_length, uint8_t *message)
extern int send_discover_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, buf_desc_t buf) {
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
extern int signal_ait_message(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, uint32_t port_id, buf_desc_t buf, uint32_t *mp, uint32_t *pp) {
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
    ANALYZE_REPLY("signal_ait_message");
    *mp = cbi.module_id;
    *pp = cbi.port_id;
}
WAIT_ACK;
    return 0;
}

extern struct nl_sock *init_sock() {
    int err;

    struct nl_sock *sock = nl_socket_alloc();
    nl_connect(sock, NETLINK_GENERIC);

    // ref: lib/genl/mngt.c
    if ((err = genl_register_family(&ops)) < 0) {
        // fatal_error(err, "Unable to register Generic Netlink family: \"%s\"", ops.o_name);
        ECP_DEBUG("genl_register_family: %d\n", err);
    }

    if ((err = genl_ops_resolve(sock, &ops)) < 0) {
        // fatal_error(err, "Unable to resolve family: \"%s\"", ops.o_name);
        ECP_DEBUG("genl_ops_resolve: %d\n", err);
    }

    ECP_DEBUG("genl_ops_resolve: \"%s\" => %d\n", ops.o_name, ops.o_id);
    ECP_DEBUG("\n");
    return sock;
}

// event listenter socket

char *GROUPS[] = { NL_ECNL_MULTICAST_GOUP_LINKSTATUS, NL_ECNL_MULTICAST_GOUP_AIT };

/* register with multicast group */
static int do_listen(struct nl_sock *sock, char *family, char *group_name) {
    int group = genl_ctrl_resolve_grp(sock, family, group_name);
    if (group < 0) { FAM_DEBUG("genl_ctrl_resolve_grp (%s) failed: %s", group_name, nl_geterror(group)); return group; }
    FAM_DEBUG("do_listen: group %s (%d)", group_name, group);
    int error = nl_socket_add_memberships(sock, group, 0);
    if (error < 0) { FAM_DEBUG("nl_socket_add_memberships failed: %d", error); return error; }
    return 0;
}

// this may be more 'proper' than init_sock() above
extern struct nl_sock *init_sock_events() {
    struct nl_sock *sock = nl_socket_alloc();
    nl_connect(sock, NETLINK_GENERIC);
    int rc = genl_ctrl_resolve(sock, ECNL_GENL_NAME);
    if (rc < 0) { FAM_DEBUG("genl_ctrl_resolve failed: %d", rc); return NULL; }

    for (int i = 0; i < ARRAY_SIZE(GROUPS); i++) {
        char *group_name = GROUPS[i];
        int rc = do_listen(sock, ECNL_GENL_NAME, group_name);
        if (rc < 0) { FAM_DEBUG("do_listen failed: %d", rc); return NULL; }
    }

    nl_socket_disable_seq_check(sock);
    return sock;
}

// int parse_generic(struct nl_cache_ops *unused, struct genl_cmd *cmd, struct genl_info *info, void *arg);
// int parse_cb(struct nl_msg *msg, void *arg);
// ANALYZE_REPLY("get_module_info(\"%s\", %d) : ", ops.o_name, ops.o_id);

extern void read_event(struct nl_sock *sock) {
    int err;
    callback_index_t cbi = { .magic = 0x5a5a };
    if ((err = nl_socket_modify_cb(sock, NL_CB_VALID, NL_CB_CUSTOM, parse_cb, &cbi)) < 0) { fatal_error(err, "nl_socket_modify_cb"); }
    if ((err = nl_recvmsgs_default(sock)) < 0) { fatal_error(err, "nl_recvmsgs_default"); }
    // cbi.xx;
}
