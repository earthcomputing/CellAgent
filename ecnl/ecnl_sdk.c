#include <netlink/attr.h>
#include <netlink/cli/utils.h>
#include <linux/genetlink.h>

#include "ecnl_sdk.h"

typedef struct {
  struct nl_sock *sock;
  struct nl_msg *msg;
  uint32_t module_id;
} nl_session_t;

typedef nl_session_t ecnl_session_t;

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

// ref: lib/nl.c
// nl_send_auto_complete(sock, msg)
// nl_recvmsgs(sk, sk->s_cb);
// nl_recvmsgs_report(sk, cb);
// if (cb->cb_recvmsgs_ow) return cb->cb_recvmsgs_ow(sk, cb); else return recvmsgs(sk, cb);
// nl_recv();

int alloc_ecnl_session(void **ecnl_session_ptr) {
    int err;
    *ecnl_session_ptr = (ecnl_session_t *) malloc(sizeof(ecnl_session_t));
    ecnl_session_t *ecnl_session = *((ecnl_session_t **) ecnl_session_ptr);
    ecnl_session->sock = nl_socket_alloc();
    struct nl_sock *sock = ecnl_session->sock;
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

    ecnl_session->msg = nlmsg_alloc();
    struct nl_msg *msg = ecnl_session->msg;
    if (msg == NULL) {
        fatal_error(NLE_NOMEM, "Unable to allocate netlink message");
    }

    // Comment out either this or setting of static module_id.  If using this, make module_id non-const.
    // char *module_name = "sim_ecnl0";
    // int rc = alloc_driver(ecnl_session->sock, ecnl_session->msg, module_name, &(ecnl_session->module_id));
    // if (rc < 0) fatal_error(rc, "alloc_driver");
    ecnl_session->module_id = 0;
};

int free_ecnl_session(void *ecnl_session) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    nl_close(sock);
    nl_socket_free(sock);
    nlmsg_free(msg);
    free((ecnl_session_t *) ecnl_session);
    return 0;
};


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
int get_link_state(struct nlattr **tb, link_state_t *lp) {
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
int get_module_info(void *ecnl_session, const module_info_t **mipp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
    *mipp = malloc(sizeof(module_info_t));
    if (*mipp != NULL) {
      module_info_t *settable_mip = (module_info_t *)(*mipp);
      settable_mip->module_id = module_id;
      settable_mip->module_name = module_name;
      settable_mip->num_ports = num_ports;
    }
}
WAIT_ACK;
    return 0;
}

// GET_PORT_STATE(uint32_t module_id, uint32_t port_id)
int get_port_state(void *ecnl_session, uint32_t port_id, uint32_t *mp, uint32_t *pp, link_state_t *lp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int alloc_table(void *ecnl_session, uint32_t table_size, uint32_t *mp, uint32_t *tp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int dealloc_table(void *ecnl_session, uint32_t table_id, uint32_t *mp, uint32_t *tp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int select_table(void *ecnl_session, uint32_t table_id, uint32_t *mp, uint32_t *tp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int fill_table(void *ecnl_session, uint32_t table_id, uint32_t table_location, uint32_t table_content_size, ecnl_table_entry_t *table_content, uint32_t *mp, uint32_t *tp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int fill_table_entry(void *ecnl_session, uint32_t table_id, uint32_t table_location, ecnl_table_entry_t *table_entry, uint32_t *mp, uint32_t *tp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int map_ports(void *ecnl_session, uint32_t *table_map, uint32_t *mp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int start_forwarding(void *ecnl_session, uint32_t *mp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int stop_forwarding(void *ecnl_session, uint32_t *mp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int read_alo_registers(void *ecnl_session, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp, uint32_t *fp, uint64_t **vp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int retrieve_ait_message(void *ecnl_session, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp, buf_desc_t *buf) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int write_alo_register(void *ecnl_session, uint32_t port_id, alo_reg_t alo_reg, uint32_t *mp, uint32_t *pp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int send_ait_message(void *ecnl_session, uint32_t port_id, buf_desc_t buf, uint32_t *mp, uint32_t *pp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int event_receive_dsc(void **tbv, uint32_t *mp, uint32_t *pp, uint8_t *dp) {
    struct nlattr **tb = (struct nlattr **) tbv;
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
int event_link_status_update(void **tbv, uint32_t *mp, uint32_t *pp, link_state_t *lp) {
    struct nlattr **tb = (struct nlattr **) tbv;
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
int event_forward_ait_message(void **tbv, uint32_t *mp, uint32_t *pp, uint32_t *lp, uint8_t *dp) {
    struct nlattr **tb = (struct nlattr **) tbv;
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
int event_got_ait_message(void **tbv, uint32_t *mp, uint32_t *pp, uint32_t *lp) {
    struct nlattr **tb = (struct nlattr **) tbv;
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
int event_got_alo_update(void **tbv, uint32_t *mp, uint32_t *pp, uint64_t *vp, uint32_t *fp) {
    struct nlattr **tb = (struct nlattr **) tbv;
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
int send_discover_message(void *ecnl_session, uint32_t port_id, buf_desc_t buf) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
int signal_ait_message(void *ecnl_session, uint32_t port_id, buf_desc_t buf, uint32_t *mp, uint32_t *pp) {
    int err;
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
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
