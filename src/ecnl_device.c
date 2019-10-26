#include <linux/err.h>
#include <linux/etherdevice.h>
#include <linux/ieee80211.h>
#include <linux/if.h>
#include <linux/if_ether.h>
#include <linux/list.h>
#include <linux/module.h>
#include <linux/netlink.h>
#include <linux/nl80211.h>
#include <linux/rtnetlink.h>
#include <linux/slab.h>

#include <net/cfg80211.h>
#include <net/genetlink.h>
#include <net/inet_connection_sock.h>
#include <net/net_namespace.h>
#include <net/sock.h>

#include <linux/kmod.h>

// intel e1000e driver i/f:

#include <e1000.h>
#include "ecnl_entl_if.h"
#include "ecnl_device.h"
#include "ecnl_protocol.h"


#define DEFMOD_DEBUG(fmt, args...) ECNL_DEBUG("%s " fmt, MAIN_DRIVER_NAME, ## args);
#define PLUG_DEBUG(plug_in, fmt, args...) ECNL_DEBUG("%s " fmt, plug_in->name, ## args);
#define ECNL_INFO(_name, fmt, args...) printk(KERN_INFO "ECNL: %s " fmt "\n", _name, ## args)


#define DRV_NAME        "ecnl"
#define DRV_VERSION     "0.0.2"

#define ECNL_DEVICE_DRIVER_VERSION "0.0.0.2"

static void dump_skbuff(void *user_hdr);

// be paranoid, probably parses with excess ';'
#define NLAPUT_CHECKED(putattr) { int rc = putattr; if (rc) return rc; }
#define NLAPUT_CHECKED_ZZ(putattr) { int rc = putattr; if (rc) return; }

static const struct genl_multicast_group nl_ecnd_mcgrps[] = {
    [NL_ECNL_MCGRP_LINKSTATUS] = { .name = NL_ECNL_MULTICAST_GOUP_LINKSTATUS },
    [NL_ECNL_MCGRP_AIT] =        { .name = NL_ECNL_MULTICAST_GOUP_AIT },
    [NL_ECNL_MCGRP_ALO] =        { .name = NL_ECNL_MULTICAST_GOUP_ALO },
    [NL_ECNL_MCGRP_DISCOVERY] =  { .name = NL_ECNL_MULTICAST_GOUP_DISCOVERY },
    [NL_ECNL_MCGRP_TEST] =       { .name = NL_ECNL_MULTICAST_GOUP_TEST },
};

static const struct nla_policy nl_ecnl_policy[NL_ECNL_ATTR_MAX+1] = {
    [NL_ECNL_ATTR_MODULE_NAME] =                { .type = NLA_NUL_STRING, .len = 20-1 },
    [NL_ECNL_ATTR_MODULE_ID] =                  { .type = NLA_U32 },
    [NL_ECNL_ATTR_NUM_PORTS] =                  { .type = NLA_U32 },
    [NL_ECNL_ATTR_PORT_ID] =                    { .type = NLA_U32 },
    [NL_ECNL_ATTR_PORT_NAME] =                  { .type = NLA_NUL_STRING, .len = 20-1 },
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

static void ecnl_setup(struct net_device *plug_in);

static struct net_device *ecnl_devices[ECNL_DRIVER_MAX];

static int num_ecnl_devices = 0;
static int device_busy = 0;

// ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in); // e_dev->ecnl_index
static struct net_device *find_ecnl_device(unsigned char *name) {
    for (int i = 0; i < num_ecnl_devices; i++) {
        struct net_device *plug_in = ecnl_devices[i];
        if (strcmp(name, plug_in->name) == 0) return plug_in;
    }
    return NULL;
}

static int nl_ecnl_pre_doit(const struct genl_ops *ops, struct sk_buff *skb, struct genl_info *info) {
    return 0;
}

static void nl_ecnl_post_doit(const struct genl_ops *ops, struct sk_buff *skb, struct genl_info *info) {
    // return 0;
}

#ifndef BIONIC
static struct genl_family nl_ecnd_fam = {
    .id = GENL_ID_GENERATE,
    .name = ECNL_GENL_NAME,
    .hdrsize = 0,
    .version = 1,
    .maxattr = NL_ECNL_ATTR_MAX,
    .netnsok = true,
    .pre_doit = nl_ecnl_pre_doit,
    .post_doit = nl_ecnl_post_doit,
};
#else
static struct genl_family nl_ecnd_fam;
#endif

#define GENLMSG_DATA(nh) ((void *) (((char *) nlmsg_data(nh)) + GENL_HDRLEN))
#define NLA_DATA(na) ((void *) (((char *) (na)) + NLA_HDRLEN))

// each call to printk() begins a new line unless KERN_CONT is used
#define PRINTIT printk

// on the user side, prefer: env NLCB=debug
static void dump_skbuff(void *user_hdr) {
#ifndef BIONIC
    struct nlmsghdr *nh = genlmsg_nlhdr(user_hdr, &nl_ecnd_fam);
#else
    struct nlmsghdr *nh = genlmsg_nlhdr(user_hdr);
#endif
    struct genlmsghdr *gh = nlmsg_data(nh);
    struct nlattr *head = (struct nlattr *) GENLMSG_DATA(nh);
    void *after = (void *) (((char *) nh) + nh->nlmsg_len);

    PRINTIT("    nh: %08lx\n", (long) nh);
    PRINTIT("    .nlmsg_len: %d\n", nh->nlmsg_len);
    PRINTIT("    .nlmsg_type: %d\n", nh->nlmsg_type);
    PRINTIT("    .nlmsg_flags: %d\n", nh->nlmsg_flags);
    PRINTIT("    .nlmsg_seq: %d\n", nh->nlmsg_seq);
    PRINTIT("    .nlmsg_pid: %d\n", nh->nlmsg_pid);

    PRINTIT("    gh: %08lx\n", (long) gh);
    PRINTIT("    .cmd: %d\n", gh->cmd);
    PRINTIT("    .version: %d\n", gh->version);
    PRINTIT("    .reserved: %d\n", gh->reserved);

    PRINTIT("\n");
    PRINTIT("    after: %08lx\n", (long) after);
    PRINTIT("    payload: %08lx\n", (long) head);

    // trusts that there's enough space for nla hdr and data
    // FIXME: after could compare against NLA_HDRLEN, then against NLMSG_ALIGN(p->nla_len)
    for (struct nlattr *p = head; after > (void *) p; p = (struct nlattr *) (((char *) p) + NLMSG_ALIGN(p->nla_len))) {
        int nbytes = p->nla_len - NLA_HDRLEN; // ((int) NLA_ALIGN(sizeof(struct nlattr)))
        void *d = NLA_DATA(p);
        PRINTIT("    nla: %08lx .nla_type: %d .nla_len: %d .data: %08lx nbytes: %d align: %d\n", (long) p, p->nla_type, p->nla_len, (long) d, nbytes, NLMSG_ALIGN(p->nla_len));
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

static ecnl_device_t *fetch_ecnl_device(struct genl_info *info) {
    if (info->attrs[NL_ECNL_ATTR_MODULE_NAME]) {
        char *dev_name = (char *) nla_data(info->attrs[NL_ECNL_ATTR_MODULE_NAME]); // nla_get_nulstring
        struct net_device *plug_in = find_ecnl_device(dev_name);
        ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);
        return e_dev;
    }

    if (info->attrs[NL_ECNL_ATTR_MODULE_ID]) {
        u32 id = nla_get_u32(info->attrs[NL_ECNL_ATTR_MODULE_ID]);
        struct net_device *plug_in = ecnl_devices[id];
        ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);
        return e_dev;
    }

    return NULL;
}

//  can be used for simulation of EC Link network in a single kernel image
static int nl_ecnl_alloc_driver(struct sk_buff *skb, struct genl_info *info) {
    if (!info->attrs[NL_ECNL_ATTR_MODULE_NAME]) return -EINVAL;

    char *dev_name = (char *) nla_data(info->attrs[NL_ECNL_ATTR_MODULE_NAME]); // nla_get_nulstring
    struct net_device *exists = find_ecnl_device(dev_name);
    if (exists) return -EINVAL;

    struct net_device *plug_in = alloc_netdev(sizeof(ecnl_device_t), dev_name, NET_NAME_UNKNOWN, ecnl_setup);
    if (!plug_in) return -ENOMEM;

    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);
    memset(e_dev, 0, sizeof(ecnl_device_t));

    int module_id = num_ecnl_devices++;
    ecnl_devices[module_id] = plug_in;
    strcpy(e_dev->ecnl_name, dev_name);
    e_dev->ecnl_index = module_id; // module_id
    spin_lock_init(&e_dev->ecnl_lock);

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_ALLOC_DRIVER);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

static int nl_ecnl_get_module_info(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    int flags = 0; // NLM_F_MULTI
    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, flags, NL_ECNL_CMD_GET_MODULE_INFO);
    // dump_skbuff(user_hdr);
    NLAPUT_CHECKED(nla_put_string(rskb, NL_ECNL_ATTR_MODULE_NAME, e_dev->ecnl_name));
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_NUM_PORTS, e_dev->ecnl_num_ports));
    genlmsg_end(rskb, user_hdr);
    // dump_skbuff(user_hdr);
    return genlmsg_reply(rskb, info);
}

static int add_link_state(struct sk_buff *rskb, ecnl_device_t *e_dev, struct entl_driver *e_driver, ec_state_t *state) {
    NLAPUT_CHECKED(nla_put_string(rskb, NL_ECNL_ATTR_MODULE_NAME, e_dev->ecnl_name));
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_PORT_ID, e_driver->eda_index)); // port_id
    NLAPUT_CHECKED(nla_put_string(rskb, NL_ECNL_ATTR_PORT_NAME, e_driver->eda_name));

    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_PORT_LINK_STATE, state->ecs_link_state));
#ifndef BIONIC
    NLAPUT_CHECKED(nla_put_u64(rskb, NL_ECNL_ATTR_PORT_S_COUNTER, state->ecs_s_count));
    NLAPUT_CHECKED(nla_put_u64(rskb, NL_ECNL_ATTR_PORT_R_COUNTER, state->ecs_r_count));
    NLAPUT_CHECKED(nla_put_u64(rskb, NL_ECNL_ATTR_PORT_RECOVER_COUNTER, state->ecs_recover_count));
    NLAPUT_CHECKED(nla_put_u64(rskb, NL_ECNL_ATTR_PORT_RECOVERED_COUNTER, state->ecs_recovered_count));
    NLAPUT_CHECKED(nla_put_u64(rskb, NL_ECNL_ATTR_PORT_ENTT_COUNT, state->ecs_recover_count));
    NLAPUT_CHECKED(nla_put_u64(rskb, NL_ECNL_ATTR_PORT_AOP_COUNT, state->ecs_recovered_count));
#else
    int padattr = 0; // FIXME ??
    NLAPUT_CHECKED(nla_put_u64_64bit(rskb, NL_ECNL_ATTR_PORT_S_COUNTER, state->ecs_s_count, padattr));
    NLAPUT_CHECKED(nla_put_u64_64bit(rskb, NL_ECNL_ATTR_PORT_R_COUNTER, state->ecs_r_count, padattr));
    NLAPUT_CHECKED(nla_put_u64_64bit(rskb, NL_ECNL_ATTR_PORT_RECOVER_COUNTER, state->ecs_recover_count, padattr));
    NLAPUT_CHECKED(nla_put_u64_64bit(rskb, NL_ECNL_ATTR_PORT_RECOVERED_COUNTER, state->ecs_recovered_count, padattr));
    NLAPUT_CHECKED(nla_put_u64_64bit(rskb, NL_ECNL_ATTR_PORT_ENTT_COUNT, state->ecs_recover_count, padattr));
    NLAPUT_CHECKED(nla_put_u64_64bit(rskb, NL_ECNL_ATTR_PORT_AOP_COUNT, state->ecs_recovered_count, padattr));
#endif
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_NUM_AIT_MESSAGES, state->ecs_num_queued));
    return 0;
}

static int nl_ecnl_get_port_state(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    // ECNL_INFO(e_dev->ecnl_name, "nl_ecnl_get_port_state");

    if (!info->attrs[NL_ECNL_ATTR_PORT_ID]) return -EINVAL;

    u32 port_id = nla_get_u32(info->attrs[NL_ECNL_ATTR_PORT_ID]);
    // ECNL_INFO(e_dev->ecnl_name, "nl_ecnl_get_port_state - port_id %d", port_id);
    if (port_id >= e_dev->ecnl_num_ports) return -EINVAL;

    struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
    if (!e_driver) return -EINVAL;
    // ECNL_INFO(e_dev->ecnl_name, "entl_driver \"%s\"", e_driver->eda_name);

    struct net_device *e1000e = e_driver->eda_device;
    struct entl_driver_funcs *funcs = e_driver->eda_funcs;
    if (!e1000e || !funcs) return -EINVAL;
    // ECNL_INFO(e_dev->ecnl_name, "e1000e: %p, funcs: %p", e1000e, funcs);
    // ECNL_INFO(e_dev->ecnl_name, "net_device: \"%s\"", e1000e->name);

    ec_state_t state; memset(&state, 0, sizeof(ec_state_t));
    int err = funcs->edf_get_state(e1000e, &state);
    if (err) return -EINVAL;

    // ECNL_INFO(e_dev->ecnl_name, "reply module_id %d", e_dev->ecnl_index);
    // ECNL_INFO(e_driver->eda_name, "reply port_id %d", e_driver->eda_index);
    // ECNL_INFO(e_driver->eda_name, "state:");

    // return data packet back to caller
    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_GET_PORT_STATE);
    NLAPUT_CHECKED(add_link_state(rskb, e_dev, e_driver, &state));
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

static int nl_ecnl_alloc_table(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    if (!info->attrs[NL_ECNL_ATTR_TABLE_SIZE]) return -EINVAL;

    u32 size = nla_get_u32(info->attrs[NL_ECNL_ATTR_TABLE_SIZE]);
    for (int id = 0; id < ENTL_TABLE_MAX; id++) {
        if (e_dev->ecnl_tables[id] != NULL) continue;

        ecnl_table_entry_t *ecnl_table = kzalloc(sizeof(struct ecnl_table_entry) * size, GFP_ATOMIC);
        if (!ecnl_table) return -ENOMEM;

        e_dev->ecnl_tables[id] = ecnl_table;
        e_dev->ecnl_tables_size[id] = size;

        struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
        if (!rskb) return -ENOMEM;

        void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_ALLOC_TABLE);
        NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
        NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_TABLE_ID, id));
        genlmsg_end(rskb, user_hdr);
        return genlmsg_reply(rskb, info);
    }

    return -EINVAL;
}

static int nl_ecnl_fill_table(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    if (!info->attrs[NL_ECNL_ATTR_TABLE_ID]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_TABLE_LOCATION]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_TABLE_CONTENT_SIZE]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_TABLE_CONTENT]) return -EINVAL;

    u32 id = nla_get_u32(info->attrs[NL_ECNL_ATTR_TABLE_ID]);
    ecnl_table_entry_t *ecnl_table = e_dev->ecnl_tables[id];
    int t_size = e_dev->ecnl_tables_size[id];

    u32 size = nla_get_u32(info->attrs[NL_ECNL_ATTR_TABLE_CONTENT_SIZE]);

    u32 location = nla_get_u32(info->attrs[NL_ECNL_ATTR_TABLE_LOCATION]);
    if (location + size > t_size) return -EINVAL;

    char *p = (char *) &ecnl_table[location];
    nla_memcpy(p, info->attrs[NL_ECNL_ATTR_TABLE_CONTENT], sizeof(struct ecnl_table_entry) * size);
    // memcpy(p, (char *) nla_data(info->attrs[NL_ECNL_ATTR_TABLE_CONTENT]), sizeof(struct ecnl_table_entry) * size); // nla_get_unspec, nla_len

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_FILL_TABLE);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_TABLE_ID, id));
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

static int nl_ecnl_fill_table_entry(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    if (!info->attrs[NL_ECNL_ATTR_TABLE_ID]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_TABLE_ENTRY_LOCATION]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_TABLE_ENTRY]) return -EINVAL;

    u32 id = nla_get_u32(info->attrs[NL_ECNL_ATTR_TABLE_ID]);
    ecnl_table_entry_t *ecnl_table = e_dev->ecnl_tables[id];
    int t_size = e_dev->ecnl_tables_size[id];

    u32 location = nla_get_u32(info->attrs[NL_ECNL_ATTR_TABLE_ENTRY_LOCATION]);
    if (location > t_size) return -EINVAL;

    ecnl_table_entry_t *p = &ecnl_table[location];
    nla_memcpy(p, info->attrs[NL_ECNL_ATTR_TABLE_ENTRY], sizeof(struct ecnl_table_entry));
    // char *entry = (char *) nla_data(info->attrs[NL_ECNL_ATTR_TABLE_ENTRY]); // nla_get_unspec, nla_len
    // memcpy((char *) &ecnl_table[location], entry, sizeof(struct ecnl_table_entry));

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_FILL_TABLE_ENTRY);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_TABLE_ID, id));
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

static int nl_ecnl_select_table(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    if (!info->attrs[NL_ECNL_ATTR_TABLE_ID]) return -EINVAL;

    u32 id = nla_get_u32(info->attrs[NL_ECNL_ATTR_TABLE_ID]);
    ecnl_table_entry_t *ecnl_table = e_dev->ecnl_tables[id];
    if (ecnl_table) {
        unsigned long flags;
        spin_lock_irqsave(&e_dev->ecnl_lock, flags);
        e_dev->ecnl_current_table = ecnl_table;
        spin_unlock_irqrestore(&e_dev->ecnl_lock, flags);
    }

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_SELECT_TABLE);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_TABLE_ID, id));
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

static int nl_ecnl_dealloc_table(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    if (!info->attrs[NL_ECNL_ATTR_TABLE_ID]) return -EINVAL;

    u32 id = nla_get_u32(info->attrs[NL_ECNL_ATTR_TABLE_ID]);
    if (e_dev->ecnl_tables[id]) {
        if (e_dev->ecnl_current_table == e_dev->ecnl_tables[id]) {
            e_dev->ecnl_fw_enable = 0;
            e_dev->ecnl_current_table = NULL;
        }
        kfree(e_dev->ecnl_tables[id]);
        e_dev->ecnl_tables[id] = NULL;
    }
    e_dev->ecnl_tables_size[id] = 0;

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_DEALLOC_TABLE);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_TABLE_ID, id));
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

// FIXME:
static int nl_ecnl_map_ports(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

// FIXME: harden this - mp length (ENCL_FW_TABLE_ENTRY_ARRAY)
    if (info->attrs[NL_ECNL_ATTR_TABLE_MAP]) {
        // char *p = &e_dev->ecnl_fw_map[0];
        // nla_memcpy(e_dev->ecnl_fw_map, info->attrs[NL_ECNL_ATTR_TABLE_MAP], sizeof(u32) * ENCL_FW_TABLE_ENTRY_ARRAY);
        u32 *mp = (u32 *) nla_data(info->attrs[NL_ECNL_ATTR_TABLE_MAP]); // nla_get_unspec, nla_len
        for (int i = 0; i < ENCL_FW_TABLE_ENTRY_ARRAY; i++) {
            e_dev->ecnl_fw_map[i] = mp[i];
        }
    }

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_MAP_PORTS);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

// UNUSED ??
static struct entl_driver *find_driver(ecnl_device_t *e_dev, char *name) {
    for (int port_id = 0; port_id < e_dev->ecnl_num_ports; port_id++) {
        struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
        if (strcmp(e_driver->eda_name, name) == 0) return e_driver;
    }
    return NULL;
}

static int nl_ecnl_start_forwarding(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    e_dev->ecnl_fw_enable = true;

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_START_FORWARDING);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

static int nl_ecnl_stop_forwarding(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    e_dev->ecnl_fw_enable = false;

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_STOP_FORWARDING);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

static char *letters = "0123456789abcdef";
static void dump_block(struct net_device *e1000e, char *tag, void *d, int nbytes) {
    char window[3*41];
    int f = 0;
    for (int i = 0; i < nbytes; i++) {
        char ch = ((char *) d)[i] & 0xff;
        int n0 = (ch & 0xf0) >> 4;
        int n1 = (ch & 0x0f);
        window[f+0] = ' ';
        window[f+1] = letters[n0];
        window[f+2] = letters[n1];
        window[f+3] = '\0';
        f += 3;
        if (f >= 3*40) break;
    }
    ECNL_INFO(e1000e->name, "%s: nbytes: %d - %s", tag, nbytes, window);
}

static int nl_ecnl_send_ait_message(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    if (!info->attrs[NL_ECNL_ATTR_PORT_ID]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_MESSAGE_LENGTH]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_MESSAGE]) return -EINVAL;

    u32 port_id = nla_get_u32(info->attrs[NL_ECNL_ATTR_PORT_ID]);

    struct ec_ait_data ait_data; memset(&ait_data, 0, sizeof(struct ec_ait_data));
    ait_data.ecad_message_len = nla_get_u32(info->attrs[NL_ECNL_ATTR_MESSAGE_LENGTH]);
    nla_memcpy(ait_data.ecad_data, info->attrs[NL_ECNL_ATTR_MESSAGE], ait_data.ecad_message_len);
    // memcpy(ait_data.ecad_data, (char *) nla_data(info->attrs[NL_ECNL_ATTR_MESSAGE]), ait_data.ecad_message_len); // nla_get_unspec, nla_len

    struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
    if (!e_driver) return -EINVAL;
    struct net_device *e1000e = e_driver->eda_device;
    struct entl_driver_funcs *funcs = e_driver->eda_funcs;
    if (!e1000e || !funcs) return -EINVAL;

// DEBUG
    dump_block(e1000e, "nl_ecnl_send", ait_data.ecad_data, ait_data.ecad_message_len);

    funcs->edf_send_AIT((struct sk_buff *) &ait_data, e1000e);

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_SEND_AIT_MESSAGE);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_PORT_ID, port_id));
    //NLAPUT_CHECKED(nla_put(rskb, NL_ECNL_ATTR_AIT_MESSAGE, sizeof(struct entt_ioctl_ait_data), ait_data));
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

static int nl_ecnl_retrieve_ait_message(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    if (!info->attrs[NL_ECNL_ATTR_PORT_ID]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_ALO_REG_DATA]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_ALO_REG_NO]) return -EINVAL;

    u32 port_id = nla_get_u32(info->attrs[NL_ECNL_ATTR_PORT_ID]);

    struct ec_alo_reg alo_reg; memset(&alo_reg, 0, sizeof(struct ec_alo_reg));
    alo_reg.ecar_reg = nla_get_u64(info->attrs[NL_ECNL_ATTR_ALO_REG_DATA]);
    alo_reg.ecar_index = nla_get_u32(info->attrs[NL_ECNL_ATTR_ALO_REG_NO]);

    struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
    if (!e_driver) return -EINVAL;
    struct net_device *e1000e = e_driver->eda_device;
    struct entl_driver_funcs *funcs = e_driver->eda_funcs;
    if (!e1000e || !funcs) return -EINVAL;

    // send/retr differ: egrep 'typedef struct (ec_ait_data|entt_ioctl_ait_data)'
    struct entt_ioctl_ait_data ait_data; memset(&ait_data, 0, sizeof(struct entt_ioctl_ait_data));
    funcs->edf_retrieve_AIT(e1000e, &ait_data);

// DEBUG
    dump_block(e1000e, "nl_ecnl_retr", ait_data.data, ait_data.message_len);
    // struct net_device *plug_in = ecnl_devices[e_dev->ecnl_index]; // module_id
    // PLUG_DEBUG(plug_in, "nl_ecnl_retr - msgs %d, nqueued %d", ait_data.num_messages, ait_data.num_queued);

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_RETRIEVE_AIT_MESSAGE);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_PORT_ID, port_id));
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MESSAGE_LENGTH, ait_data.message_len));
    NLAPUT_CHECKED(nla_put(rskb, NL_ECNL_ATTR_MESSAGE, ait_data.message_len, ait_data.data));
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

static int nl_ecnl_write_alo_register(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    struct nlattr *na;
    if (!info->attrs[NL_ECNL_ATTR_PORT_ID]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_ALO_REG_DATA]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_ALO_REG_NO]) return -EINVAL;

    u32 port_id = nla_get_u32(info->attrs[NL_ECNL_ATTR_PORT_ID]);

    struct ec_alo_reg alo_reg; memset(&alo_reg, 0, sizeof(struct ec_alo_reg));
    alo_reg.ecar_reg = nla_get_u64(info->attrs[NL_ECNL_ATTR_ALO_REG_DATA]);
    alo_reg.ecar_index = nla_get_u32(info->attrs[NL_ECNL_ATTR_ALO_REG_NO]);

    struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
    if (!e_driver) return -EINVAL;
    struct net_device *e1000e = e_driver->eda_device;
    struct entl_driver_funcs *funcs = e_driver->eda_funcs;
    if (!e1000e || !funcs) return -EINVAL;

    funcs->edf_write_reg(e1000e, &alo_reg);

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_WRITE_ALO_REGISTER);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_PORT_ID, port_id));
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

static int nl_ecnl_read_alo_registers(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    if (!info->attrs[NL_ECNL_ATTR_PORT_ID]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_ALO_REG_DATA]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_ALO_REG_NO]) return -EINVAL;

    u32 port_id = nla_get_u32(info->attrs[NL_ECNL_ATTR_PORT_ID]);

    struct ec_alo_reg alo_reg; memset(&alo_reg, 0, sizeof(struct ec_alo_reg));
    alo_reg.ecar_reg = nla_get_u64(info->attrs[NL_ECNL_ATTR_ALO_REG_DATA]);
    alo_reg.ecar_index = nla_get_u32(info->attrs[NL_ECNL_ATTR_ALO_REG_NO]);

    struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
    if (!e_driver) return -EINVAL;
    struct net_device *e1000e = e_driver->eda_device;
    struct entl_driver_funcs *funcs = e_driver->eda_funcs;
    if (!e1000e || !funcs) return -EINVAL;

    struct ec_alo_regs alo_regs; memset(&alo_regs, 0, sizeof(struct ec_alo_regs));
    funcs->edf_read_regset(e1000e, &alo_regs);

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, info->snd_portid, info->snd_seq, &nl_ecnd_fam, 0, NL_ECNL_CMD_READ_ALO_REGISTERS);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, e_dev->ecnl_index)); // module_id
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_PORT_ID, port_id));
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_ALO_FLAG, alo_regs.ecars_flags));
    NLAPUT_CHECKED(nla_put(rskb, NL_ECNL_ATTR_ALO_REG_VALUES, sizeof(uint64_t)*32, alo_regs.ecars_regs));
    genlmsg_end(rskb, user_hdr);
    return genlmsg_reply(rskb, info);
}

static int nl_ecnl_send_discover_message(struct sk_buff *skb, struct genl_info *info) {
    ecnl_device_t *e_dev = fetch_ecnl_device(info);
    if (!e_dev) return -ENODEV;

    if (!info->attrs[NL_ECNL_ATTR_PORT_ID]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_DISCOVERING_MSG]) return -EINVAL;
    if (!info->attrs[NL_ECNL_ATTR_MESSAGE_LENGTH]) return -EINVAL;

    u32 port_id = nla_get_u32(info->attrs[NL_ECNL_ATTR_PORT_ID]);

    u32 len = nla_get_u32(info->attrs[NL_ECNL_ATTR_MESSAGE_LENGTH]);
    struct sk_buff *rskb = alloc_skb(len,  GFP_ATOMIC); if (rskb == NULL) return -ENOMEM;
    rskb->len = len;

    nla_memcpy(rskb->data, info->attrs[NL_ECNL_ATTR_DISCOVERING_MSG], len);
    // char *data = (char *) nla_data(info->attrs[NL_ECNL_ATTR_DISCOVERING_MSG]); // nla_get_unspec, nla_len
    // memcpy(rskb->data, data, len);

    struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
    if (!e_driver) return -EINVAL;
    struct net_device *e1000e = e_driver->eda_device;
    struct entl_driver_funcs *funcs = e_driver->eda_funcs;
    if (!e1000e || !funcs) return -EINVAL;

    funcs->edf_start_xmit(rskb, e1000e);
    return -1; // FIXME
}

static const struct genl_ops nl_ecnl_ops[] = {
    {
        .cmd = NL_ECNL_CMD_ALLOC_DRIVER,
        .doit = nl_ecnl_alloc_driver,
        .policy = nl_ecnl_policy,
    },
    {
        .cmd = NL_ECNL_CMD_GET_MODULE_INFO,
        .doit = nl_ecnl_get_module_info,
        .policy = nl_ecnl_policy,
        /* can be retrieved by unprivileged users */
    },
    {
        .cmd = NL_ECNL_CMD_GET_PORT_STATE,
        .doit = nl_ecnl_get_port_state,
        .policy = nl_ecnl_policy,
        /* can be retrieved by unprivileged users */
    },
    {
        .cmd = NL_ECNL_CMD_ALLOC_TABLE,
        .doit = nl_ecnl_alloc_table,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },
    {
        .cmd = NL_ECNL_CMD_FILL_TABLE,
        .doit = nl_ecnl_fill_table,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },
    {
        .cmd = NL_ECNL_CMD_FILL_TABLE_ENTRY,
        .doit = nl_ecnl_fill_table_entry,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },
    {
        .cmd = NL_ECNL_CMD_SELECT_TABLE,
        .doit = nl_ecnl_select_table,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },
    {
        .cmd = NL_ECNL_CMD_DEALLOC_TABLE,
        .doit = nl_ecnl_dealloc_table,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },
    {
        .cmd = NL_ECNL_CMD_MAP_PORTS,
        .doit = nl_ecnl_map_ports,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },
    {
        .cmd = NL_ECNL_CMD_START_FORWARDING,
        .doit = nl_ecnl_start_forwarding,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },
    {
        .cmd = NL_ECNL_CMD_STOP_FORWARDING,
        .doit = nl_ecnl_stop_forwarding,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },
    {
        .cmd = NL_ECNL_CMD_SEND_AIT_MESSAGE,
        .doit = nl_ecnl_send_ait_message,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },
    {
        .cmd = NL_ECNL_CMD_SIGNAL_AIT_MESSAGE,
        .doit = nl_ecnl_send_ait_message,  // dummy func
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },
    {
        .cmd = NL_ECNL_CMD_RETRIEVE_AIT_MESSAGE,
        .doit = nl_ecnl_retrieve_ait_message,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },
    {
        .cmd = NL_ECNL_CMD_WRITE_ALO_REGISTER,
        .doit = nl_ecnl_write_alo_register,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },
    {
        .cmd = NL_ECNL_CMD_READ_ALO_REGISTERS,
        .doit = nl_ecnl_read_alo_registers,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },

    {
        .cmd = NL_ECNL_CMD_SEND_DISCOVER_MESSAGE,
        .doit = nl_ecnl_send_discover_message,
        .policy = nl_ecnl_policy,
        .flags = GENL_ADMIN_PERM,
    },

};

#ifdef BIONIC
static struct genl_family nl_ecnd_fam = {
    // .id = GENL_ID_GENERATE,
    .name = ECNL_GENL_NAME,
    .hdrsize = 0,
    .version = 1,
    .maxattr = NL_ECNL_ATTR_MAX,
    .netnsok = true,
    .pre_doit = nl_ecnl_pre_doit,
    .post_doit = nl_ecnl_post_doit,
    .ops = nl_ecnl_ops,
    .n_ops = ARRAY_SIZE(nl_ecnl_ops),
    .mcgrps = nl_ecnd_mcgrps,
    .n_mcgrps = ARRAY_SIZE(nl_ecnd_mcgrps),
};
#endif

// unused ?
static int ecnl_driver_index(unsigned char *ecnl_name) {
    struct net_device *plug_in = find_ecnl_device(ecnl_name);
    if (plug_in == NULL) {
        DEFMOD_DEBUG("ecnl_driver_index - module \"%s\" not found", ecnl_name);
        return -EINVAL;
    }

    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);
    return e_dev->ecnl_index; // module_id
}

static int ecnl_register_port(int module_id, unsigned char *name, struct net_device *e1000e, struct entl_driver_funcs *funcs) {
    struct net_device *plug_in = ecnl_devices[module_id];
    if (plug_in == NULL) {
        DEFMOD_DEBUG("ecnl_register_port - module-id %d not found", module_id);
        return -EINVAL;
    }

    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);

    int port_id = -1;
    unsigned long flags;
    spin_lock_irqsave(&e_dev->ecnl_lock, flags);
    // PLUG_DEBUG(plug_in, "ecnl_register_port - port-name \"%s\" port-id %d", name, e_dev->ecnl_num_ports);
    if (e_dev->ecnl_num_ports < ECNL_DRIVER_MAX) {
        // FIXME: ENCL_FW_TABLE_ENTRY_ARRAY
        port_id = e_dev->ecnl_num_ports++;

        e_dev->ecnl_fw_map[port_id] = port_id; // default map by register order

        PLUG_DEBUG(plug_in, "ecnl_register_port - port-name \"%s\" port-id %d", name, port_id);
        struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
        e_driver->eda_index = port_id; // port_id
        e_driver->eda_name = name;
        e_driver->eda_device = e1000e;
        e_driver->eda_funcs = funcs;
    }
    else {
        PLUG_DEBUG(plug_in, "ecnl_register_port - table overflow %d", e_dev->ecnl_num_ports);
    }
    spin_unlock_irqrestore(&e_dev->ecnl_lock, flags);

    return port_id;
}

static void ecnl_deregister_ports(int module_id) {
    struct net_device *plug_in = ecnl_devices[module_id];
    if (plug_in == NULL) {
        DEFMOD_DEBUG("ecnl_deregister_ports - module-id %d not found", module_id);
        return;
    }

    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);

    unsigned long flags;
    spin_lock_irqsave(&e_dev->ecnl_lock, flags);
    e_dev->ecnl_num_ports = 0;
    spin_unlock_irqrestore(&e_dev->ecnl_lock, flags);
}

/*
static int ecnl_hard_header(struct sk_buff *skb, struct net_device *plug_in, unsigned short type, void *daddr, void *saddr, unsigned len) {
    return 0;
}

static int ecnl_rebuild_header(struct sk_buff *skb) {
    return 0;
}

static u8 ecnl_table_lookup(ecnl_device_t *e_dev, u16 u_addr, u32 l_addr) {
    if (e_dev->ecnl_hash_enable) {
        u16 hash_entry = addr_hash_10(u_addr, l_addr);
        while (1) {
            struct ecnl_hash_entry *h_e = &e_dev->ecnl_current_hash_table[hash_entry];
            if (h_e->u_addr == u_addr && h_e->l_addr == l_addr) {
                u32 offset = h_e->location & 0xf;
                ecnl_table_entry e =  e_dev->ecnl_current_table[h_e->location >> 4];
                return (e >> offset) & 0xf;
            }
            if (h_e->next == 0) return 0xff; // not found
            hash_entry = h_e->next;
        }
    }
    else {
        u32 offset = l_addr & 0xf;
        ecnl_table_entry e = e_dev->ecnl_current_table[l_addr>>4];
        return (e >> offset) & 0xf;
    }
}
*/

/*
 *	This is an Ethernet frame header.
 */

//struct ethhdr {
//	unsigned char	h_dest[ETH_ALEN];	/* destination eth addr	*/
//	unsigned char	h_source[ETH_ALEN];	/* source ether addr	*/
//	__be16		h_proto;		/* packet type ID field	*/
//} __attribute__((packed));

static void set_next_id(struct sk_buff *skb, u32 nextID) {
    struct ethhdr *eth = (struct ethhdr *) skb->data;
    eth->h_source[2] = 0xff & (nextID >> 24);
    eth->h_source[3] = 0xff & (nextID >> 16);
    eth->h_source[4] = 0xff & (nextID >>  8);
    eth->h_source[5] = 0xff & (nextID);
}

static void fetch_entry(ecnl_device_t *e_dev, u32 id, ecnl_table_entry_t *entry) {
    unsigned long flags;
    spin_lock_irqsave(&e_dev->ecnl_lock, flags);
    memcpy(entry, &e_dev->ecnl_current_table[id], sizeof(ecnl_table_entry_t));
    spin_unlock_irqrestore(&e_dev->ecnl_lock, flags);
}

static int ecnl_receive_skb(int module_id, int index, struct sk_buff *skb) {
    struct net_device *plug_in = ecnl_devices[module_id];
    if (plug_in == NULL) {
        DEFMOD_DEBUG("ecnl_receive_skb - module-id %d not found", module_id);
        return -EINVAL;
    }

    struct ethhdr *eth = (struct ethhdr *) skb->data;

    // no forwarding, send to host
    u8 dest_fw = eth->h_dest[0] & 0x80;
    if (dest_fw == 0) {
        netif_rx(skb);
        return 0;
    }

    // forwarding disabled, send to host
    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);
    if (!e_dev->ecnl_fw_enable) {
        netif_rx(skb);
        return 0;
    }

    u8 host_on_backward = eth->h_source[0] & 0x40;
    u8 direction =        eth->h_source[0] & 0x80;
    u32 id =  (u32) eth->h_source[2] << 24
            | (u32) eth->h_source[3] << 16
            | (u32) eth->h_source[4] << 8
            | (u32) eth->h_source[5];

    // table miss, send to host
    if (!e_dev->ecnl_current_table || id >= e_dev->ecnl_current_table_size) {
        PLUG_DEBUG(plug_in, "ecnl_receive_skb - can't forward packet id %d", id);
        netif_rx(skb);
        return 0;
    }

    ecnl_table_entry_t entry; // FIXME
    fetch_entry(e_dev, id, &entry);

    u16 port_vector = entry.info.port_vector;
    if (direction == 0) {  // forward direction
        if (port_vector == 0) {
            PLUG_DEBUG(plug_in, "ecnl_receive_skb no forward bit %08x", index);
            return -EINVAL;
        }

        // send to this host
        if (port_vector & 1) {
            if (port_vector & 0xfffe) {
                struct sk_buff *skbc = skb_clone(skb, GFP_ATOMIC);
                netif_rx(skbc);
            }
            else netif_rx(skb);
        }

        // multi-port forwarding
        port_vector &= ~(u16)(1 << index); // avoid to send own port
        port_vector = (port_vector >> 1);  // reduce host bit

        for (int i = 0; i < ENCL_FW_TABLE_ENTRY_ARRAY; i++) {
            if (port_vector & 1) {
                int port_id = e_dev->ecnl_fw_map[i];
                u32 nextID = entry.nextID[port_id];

                // FIXME : error or warning ??
                struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
                if (!e_driver) continue;
                struct net_device *e1000e = e_driver->eda_device;
                struct entl_driver_funcs *funcs = e_driver->eda_funcs;
                if (!e1000e || !funcs) continue;

                // forwarding to next e_driver
                if (port_vector & 0xfffe) {
                    struct sk_buff *skbc = skb_clone(skb, GFP_ATOMIC);
                    set_next_id(skbc, nextID);
                    funcs->edf_start_xmit(skbc, e1000e);
                }
                else {
                    set_next_id(skb, nextID);
                    funcs->edf_start_xmit(skb, e1000e);
                }
            }
            port_vector = port_vector >> 1;
        }
    }
    else { // backward
        u8 parent = entry.info.parent;
        // send to this host
        if (parent == 0 || host_on_backward) {
            if (parent > 0) {
                struct sk_buff *skbc = skb_clone(skb, GFP_ATOMIC);
                netif_rx(skbc);
            }
            else netif_rx(skb);
        }
// FIXME: harden against ENCL_FW_TABLE_ENTRY_ARRAY ??
        if (parent > 0) {
            int port_id = e_dev->ecnl_fw_map[parent];
            u32 nextID = entry.nextID[port_id];
            set_next_id(skb, nextID);

            struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
            if (!e_driver) return -EINVAL;
            struct net_device *e1000e = e_driver->eda_device;
            struct entl_driver_funcs *funcs = e_driver->eda_funcs;
            if (!e1000e || !funcs) return -EINVAL;

            funcs->edf_start_xmit(skb, e1000e);
        }
    }

    return 0;
}


// PUB/SUB section:


// entl e_driver received a discovery message
static int ecnl_receive_dsc(int module_id, int index, struct sk_buff *skb) {
    struct ethhdr *eth = (struct ethhdr *) skb->data;
    struct net_device *plug_in = ecnl_devices[module_id];
    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, 0, 0, &nl_ecnd_fam, 0, NL_ECNL_CMD_RETRIEVE_AIT_MESSAGE);
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED(nla_put_u32(rskb, NL_ECNL_ATTR_PORT_ID, index));
    NLAPUT_CHECKED(nla_put(rskb, NL_ECNL_ATTR_DISCOVERING_MSG, skb->len, skb->data));
    genlmsg_end(rskb, user_hdr);
    return genlmsg_multicast_allns(&nl_ecnd_fam, rskb, 0, NL_ECNL_MCGRP_DISCOVERY, GFP_KERNEL);
}

// entl e_driver has a link update
static void ecnl_link_status_update(int module_id, int port_id, ec_state_t *state) {
    struct net_device *plug_in = ecnl_devices[module_id];
    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);
    struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return; // -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, 0, 0, &nl_ecnd_fam, 0, NL_ECNL_CMD_GET_PORT_STATE);
    NLAPUT_CHECKED_ZZ(add_link_state(rskb, e_dev, e_driver, state));
    genlmsg_end(rskb, user_hdr);
    genlmsg_multicast_allns(&nl_ecnd_fam, rskb, 0, NL_ECNL_MCGRP_LINKSTATUS, GFP_KERNEL);
}

static void ecnl_forward_ait_message(int module_id, int drv_index, struct sk_buff *skb) {
    struct net_device *plug_in = ecnl_devices[module_id];
    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);

    struct ethhdr *eth = (struct ethhdr *) skb->data;
    u32 alo_command = (uint32_t) eth->h_dest[2] << 8
                    | (uint32_t) eth->h_dest[3];

    u8 to_host;
    if ((eth->h_dest[0] & 0x80) != 0) {  // fw bit set
        u8 host_on_backward = eth->h_source[0] & 0x40;
        u8 direction =        eth->h_source[0] & 0x80;
        u32 id =  (u32) eth->h_source[2] << 24
                | (u32) eth->h_source[3] << 16
                | (u32) eth->h_source[4] <<  8
                | (u32) eth->h_source[5];

        if (e_dev->ecnl_fw_enable
        &&  e_dev->ecnl_current_table && id < e_dev->ecnl_current_table_size) {
            u16 port_vector;

            ecnl_table_entry_t entry; // FIXME
            fetch_entry(e_dev, id, &entry);

            port_vector = entry.info.port_vector;
            if (direction == 0) {  // forward direction
                if (port_vector == 0) {
                    PLUG_DEBUG(plug_in, "ecnl_forward_ait_message - no forward bit xx %08x", drv_index);
                    to_host = 1;
                }
                else {
                    if (port_vector & 1) to_host = 1;
                    port_vector &= ~(u16)(1 << drv_index); // avoid to send own port
                    port_vector >>= 1; // reduce host bit

                    // device-index, NOT port-index!!
                    for (int i = 0; i < ENCL_FW_TABLE_ENTRY_ARRAY && port_vector > 0; i++, port_vector >>= 1) {
                        if ((port_vector & 1) == 0) continue;

                        int port_id = e_dev->ecnl_fw_map[i];
                        u32 nextID = entry.nextID[port_id];

                        // FIXME : error or warning ??
                        struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
                        if (!e_driver) continue;
                        struct net_device *e1000e = e_driver->eda_device;
                        struct entl_driver_funcs *funcs = e_driver->eda_funcs;
                        if (!e1000e || !funcs) continue;

                        struct sk_buff *skbc = skb_clone(skb, GFP_ATOMIC);
                        set_next_id(skbc, nextID);
                        funcs->edf_send_AIT(skbc, e1000e);
                    }
                }
            }
            else {
// FIXME: harden against ENCL_FW_TABLE_ENTRY_ARRAY ??
                // backword transfer
                u8 parent = entry.info.parent;
                if (parent == 0 || host_on_backward) {
                    to_host = 1;
                }
                int module_id = e_dev->ecnl_index;
                if (parent > 0 && module_id != parent) {
                    int port_id = e_dev->ecnl_fw_map[parent];
                    u32 nextID = entry.nextID[port_id];

                    struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
                    if (!e_driver) return; // -1;
                    struct net_device *e1000e = e_driver->eda_device;
                    struct entl_driver_funcs *funcs = e_driver->eda_funcs;
                    if (!e1000e || !funcs) return; // -1;

                    struct sk_buff *skbc = skb_clone(skb, GFP_ATOMIC);
                    set_next_id(skbc, nextID);
                    funcs->edf_send_AIT(skbc, e1000e);
                }
            }
        }
    }

    if (to_host && alo_command == 0) {  // do not forward ALO operation message
        struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
        if (!rskb) return; // -ENOMEM;

        void *user_hdr = genlmsg_put(rskb, 0, 0, &nl_ecnd_fam, 0, NL_ECNL_CMD_RETRIEVE_AIT_MESSAGE);
        NLAPUT_CHECKED_ZZ(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, module_id));
        NLAPUT_CHECKED_ZZ(nla_put_u32(rskb, NL_ECNL_ATTR_PORT_ID, drv_index));
        NLAPUT_CHECKED_ZZ(nla_put_u32(rskb, NL_ECNL_ATTR_MESSAGE_LENGTH, skb->len));
        NLAPUT_CHECKED_ZZ(nla_put(rskb, NL_ECNL_ATTR_MESSAGE, skb->len, skb->data));
        genlmsg_end(rskb, user_hdr);
        genlmsg_multicast_allns(&nl_ecnd_fam, rskb, 0, NL_ECNL_MCGRP_AIT, GFP_KERNEL);
        return;
    }

    return; // -1;
}

static void ecnl_got_ait_message(int module_id, int port_id, int num_message) {
    //struct ethhdr *eth = (struct ethhdr *) skb->data;
    struct net_device *plug_in = ecnl_devices[module_id];
    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);
    struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return; // -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, 0, 0, &nl_ecnd_fam, 0, NL_ECNL_CMD_SIGNAL_AIT_MESSAGE);
    NLAPUT_CHECKED_ZZ(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED_ZZ(nla_put_u32(rskb, NL_ECNL_ATTR_PORT_ID, port_id));
    NLAPUT_CHECKED_ZZ(nla_put_u32(rskb, NL_ECNL_ATTR_NUM_AIT_MESSAGES, num_message));
    genlmsg_end(rskb, user_hdr);
    genlmsg_multicast_allns(&nl_ecnd_fam, rskb, 0, NL_ECNL_MCGRP_AIT, GFP_KERNEL);
}

static void ecnl_got_alo_update(int module_id, int port_id) {
    //struct ethhdr *eth = (struct ethhdr *) skb->data;
    struct net_device *plug_in = ecnl_devices[module_id];
    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);

    struct ec_alo_regs alo_regs; memset(&alo_regs, 0, sizeof(struct ec_alo_regs));

    struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
    if (!e_driver) return; // -1;
    struct net_device *e1000e = e_driver->eda_device;
    struct entl_driver_funcs *funcs = e_driver->eda_funcs;
    if (!e1000e || !funcs) return; // -1;

    funcs->edf_read_regset(e1000e, &alo_regs);

    struct sk_buff *rskb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!rskb) return; // -ENOMEM;

    void *user_hdr = genlmsg_put(rskb, 0, 0, &nl_ecnd_fam, 0, NL_ECNL_CMD_READ_ALO_REGISTERS);
    NLAPUT_CHECKED_ZZ(nla_put_u32(rskb, NL_ECNL_ATTR_MODULE_ID, module_id));
    NLAPUT_CHECKED_ZZ(nla_put_u32(rskb, NL_ECNL_ATTR_PORT_ID, port_id));
    NLAPUT_CHECKED_ZZ(nla_put(rskb, NL_ECNL_ATTR_ALO_REG_VALUES, sizeof(uint64_t)*32, alo_regs.ecars_regs));
    NLAPUT_CHECKED_ZZ(nla_put_u32(rskb, NL_ECNL_ATTR_ALO_FLAG, alo_regs.ecars_flags));
    genlmsg_end(rskb, user_hdr);
    genlmsg_multicast_allns(&nl_ecnd_fam, rskb, 0, NL_ECNL_MCGRP_AIT, GFP_KERNEL);
}

static struct ecnl_funcs ecnl_api_funcs = {
    .ecnlf_register_port = ecnl_register_port,
    .ecnlf_deregister_ports = ecnl_deregister_ports,
    .ecnlf_receive_skb = ecnl_receive_skb,
    //.ecnlf_receive_dsc = ecnl_receive_dsc,
    .ecnlf_link_status_update = ecnl_link_status_update,
    .ecnlf_forward_ait_message = ecnl_forward_ait_message,
    .ecnlf_got_ait_massage = ecnl_got_ait_message,
    .ecnlf_got_alo_update = ecnl_got_alo_update
};

EXPORT_SYMBOL(ecnl_api_funcs);

// net_device interface functions
static int ecnl_open(struct net_device *plug_in) {
    PLUG_DEBUG(plug_in, "ecnl_open");
    netif_start_queue(plug_in);
    return 0;
}

static int ecnl_stop(struct net_device *plug_in) {
    PLUG_DEBUG(plug_in, "ecnl_stop");
    netif_stop_queue(plug_in);
    return 0;
}

static int ecnl_hard_start_xmit(struct sk_buff *skb, struct net_device *plug_in) {
    struct ethhdr *eth = (struct ethhdr *) skb->data;
    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);

    if (!e_dev->ecnl_fw_enable) return -EINVAL;

    u8 direction =  eth->h_source[0] & 0x80;
    u32 id =  (u32) eth->h_source[2] << 24
            | (u32) eth->h_source[3] << 16
            | (u32) eth->h_source[4] <<  8
            | (u32) eth->h_source[5];

    // FIXME: direct mapped table ??

    if (!e_dev->ecnl_current_table || id >= e_dev->ecnl_current_table_size) {
        PLUG_DEBUG(plug_in, "ecnl_hard_start_xmit \"%s\" can't forward packet", e_dev->ecnl_name);
        return -EINVAL;
    }

    ecnl_table_entry_t entry; // FIXME
    fetch_entry(e_dev, id, &entry);

    u16 port_vector = entry.info.port_vector;
    if (direction == 0) {  // forward direction
        port_vector >>= 1; // remove host bit

        for (int i = 0; i < ENCL_FW_TABLE_ENTRY_ARRAY; i++) {
            if (port_vector & 1) {
                int port_id = e_dev->ecnl_fw_map[i];
                u32 nextID = entry.nextID[port_id];

                struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
                if (!e_driver) return -EINVAL;
                struct net_device *e1000e = e_driver->eda_device;
                struct entl_driver_funcs *funcs = e_driver->eda_funcs;
                if (!e1000e || !funcs) return -EINVAL;

                // forwarding to next e_driver
                if (port_vector & 0xfffe) {
                    struct sk_buff *skbc = skb_clone(skb, GFP_ATOMIC);
                    set_next_id(skbc, nextID);
                    funcs->edf_start_xmit(skbc, e1000e);
                }
                else {
                    set_next_id(skb, nextID);
                    funcs->edf_start_xmit(skb, e1000e);
                }
            }
            port_vector = (port_vector >> 1);
        }
    }
    else {  // to parent side
        u8 parent = entry.info.parent;
        if (parent == 0) {
            // send to this host
            PLUG_DEBUG(plug_in, "ecnl_hard_start_xmit \"%s\" forwarding packet to self", e_dev->ecnl_name);
            return -EINVAL;
        }
        else {
// FIXME: harden against ENCL_FW_TABLE_ENTRY_ARRAY ??
            int port_id = e_dev->ecnl_fw_map[parent];
            u32 nextID = entry.nextID[port_id];
            set_next_id(skb, nextID);

            struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
            if (!e_driver) return -EINVAL;
            struct net_device *e1000e = e_driver->eda_device;
            struct entl_driver_funcs *funcs = e_driver->eda_funcs;
            if (!e1000e || !funcs) return -EINVAL;

            // forwarding to next e_driver
            funcs->edf_start_xmit(skb, e1000e);
        }
    }

    return 0;
}

static void ecnl_tx_timeout(struct net_device *plug_in) {
    // return 0;
}

static struct net_device_stats *ecnl_get_stats(struct net_device *plug_in) {
    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);
    return &e_dev->ecnl_net_stats;
}

static struct net_device_ops ecnl_netdev_ops = {
    .ndo_open = ecnl_open,
    .ndo_stop = ecnl_stop,
    .ndo_start_xmit = ecnl_hard_start_xmit,
    .ndo_tx_timeout = ecnl_tx_timeout,
    .ndo_get_stats = ecnl_get_stats
};

// --

#if 0
// The data structre represents the internal state of ENTL
typedef struct ec_state {
    uint64_t recover_count; // how many recover happened
    uint64_t recovered_count; // how many recovered happened
    uint64_t s_count; // how many s message happened
    uint64_t r_count; // how many r message happened
    uint64_t entt_count; // how many entt transaction happened
    uint64_t aop_count; // how many aop transaction happened
    int link_state; // link state
    int num_queued; // num AIT messages
    struct timespec update_time; // last updated time in microsecond resolution
#ifdef ENTL_SPEED_CHECK
    struct timespec interval_time; // the last interval time between S <-> R transition
    struct timespec max_interval_time; // the max interval time
    struct timespec min_interval_time; // the min interval time
#endif
} ec_state_t;

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

typedef struct entl_device {
    entl_state_machine_t stm;
    struct timer_list watchdog_timer;
    struct work_struct watchdog_task;
    int user_pid; // subscribed listener
    __u32 flag; // used to set a request to the service task
    __u16 u_addr; __u32 l_addr; // last value - for retry
    int action;
    char name[ENTL_DEVICE_NAME_LEN];
    ENTL_skb_queue_t tx_skb_queue;
    int queue_stopped;
} entl_device_t;

#endif

typedef struct {
    int index;
    unsigned char *name;
    struct net_device *e1000e;
    int port_id;
} e1000e_hackery_t;

// #define ARRAY_SIZE(X) (sizeof(X) / sizeof((X)[0])) // <linux/kernel.h>

// map: name -> instance (e1000e)
e1000e_hackery_t e1000e_ports[] = {
    { .index = 1, .name = "enp6s0", .e1000e = NULL, .port_id = -1 },
    { .index = 2, .name = "enp8s0", .e1000e = NULL, .port_id = -1 },
    { .index = 3, .name = "enp9s0", .e1000e = NULL, .port_id = -1 },
    { .index = 4, .name = "enp7s0", .e1000e = NULL, .port_id = -1 },
    { .index = 5, .name = "eno1",   .e1000e = NULL, .port_id = -1 },
};

// FIXME : should instead auto-detect compatible instances
// ref: scan_netdev - at init time search all devices to find instance we like/support
static void inject_dev(struct net_device *n_dev) {
    for (int i = 0; i < ARRAY_SIZE(e1000e_ports); i++) {
        e1000e_hackery_t *hack = &e1000e_ports[i];
        if (strcmp(n_dev->name, hack->name) == 0) { hack->e1000e = n_dev; } // inject reference
    }
}

typedef struct entl_mgr_plus {
    struct entl_mgr emp_base; // callback struct
    struct net_device *emp_plug_in; // ecnl instance
    e1000e_hackery_t *emp_hack;
} entl_mgr_plus_t;

// void (*emf_event)(struct entl_mgr *self, int sigusr); // called from watchdog, be careful
static void adapt_event(struct entl_mgr *self, int sigusr) {
    entl_mgr_plus_t *priv = (entl_mgr_plus_t *) self;
    struct net_device *plug_in = priv->emp_plug_in; // ecnl instance
    e1000e_hackery_t *hack = priv->emp_hack;

    PLUG_DEBUG(plug_in, "event %d \"%s\"", sigusr, hack->name);

    // int port_id = hack->index; // WRONG!
    int port_id = hack->port_id;
    // char *name = hack->name;
    struct net_device *e1000e = hack->e1000e;

    PLUG_DEBUG(plug_in, "event %d \"%s\" (%d)", sigusr, hack->name, port_id);

    // struct net_device *plug_in = ecnl_devices[module_id];
    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);
    if (!e_dev) return;

    int module_id = e_dev->ecnl_index;
    struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
    if (!e_driver) return;

    PLUG_DEBUG(plug_in, "event %d \"%s\" (%d) - module %d check: %s (%d)", sigusr, hack->name, port_id, module_id, e_driver->eda_name, e_driver->eda_index);

    // struct net_device *e1000e = e_driver->eda_device;
    // int port_id = e_driver->eda_index;
    // char *name = e_driver->eda_name;
    struct entl_driver_funcs *funcs = e_driver->eda_funcs;
    if (!funcs) return;

    // ENTL_DEVICE_FLAG_SIGNAL, i.e. link up/down, fatal error
    if (sigusr == SIGUSR1 /*10*/) {
        ec_state_t state; memset(&state, 0, sizeof(ec_state_t));
        int err = funcs->edf_get_state(e1000e, &state);
        if (!err) {
            PLUG_DEBUG(plug_in, "event %d \"%s\" (%d) - %d link: %d", sigusr, hack->name, port_id, module_id, state.ecs_link_state);
            PLUG_DEBUG(plug_in, "event -"
                // PRIu64 - %llu - <inttypes.h>
                " recover_count %llu"
                " recovered_count %llu"
                " s_count %llu"
                " r_count %llu"
                " entt_count %llu"
                " aop_count %llu"
                " link_state %d"
                " num_queued %d"
                // " update_time"
                ,
                state.ecs_recover_count,
                state.ecs_recovered_count,
                state.ecs_s_count,
                state.ecs_r_count,
                state.ecs_entt_count,
                state.ecs_aop_count,
                state.ecs_link_state,
                state.ecs_num_queued
                // timespec state.ecs_update_time,
            );
// FIXME
            // ecnl_link_status_update(module_id, port_id, &state); // NL_ECNL_MCGRP_LINKSTATUS
        }
    }

    // ENTL_DEVICE_FLAG_SIGNAL2, process_tx_packet, process_rx_packet
    if (sigusr == SIGUSR2 /*12*/) {
        int num_message = 1;
        // ecnl_got_ait_message(module_id, port_id, num_message); // NL_ECNL_MCGRP_AIT
    }

    // multicast
    // ecnl_receive_dsc(int module_id, int index, struct sk_buff *skb); // NL_ECNL_MCGRP_DISCOVERY
    // ecnl_forward_ait_message(int module_id, int drv_index, struct sk_buff *skb); // NL_ECNL_MCGRP_AIT
    // ecnl_got_alo_update(int module_id, int index); // NL_ECNL_MCGRP_AIT
}

// FIXME: validate here
// once we've found e1000e instances, connect plug_in (ecnl_device_t) & e1000e (entl_device_t)
static void hack_init(struct net_device *plug_in) {
    ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in); // e_dev->ecnl_index
    int module_id = e_dev->ecnl_index;

    struct entl_driver_funcs *funcs = &entl_adapt_funcs; // backdoor into e1000e/entl (for ecnl use)

    for (int i = 0; i < ARRAY_SIZE(e1000e_ports); i++) {
        e1000e_hackery_t *hack = &e1000e_ports[i];

        struct net_device *e1000e = hack->e1000e;
        if (!e1000e) continue;

        // data structures from e1000e driver side:
        // struct e1000_adapter *adapter = netdev_priv(e1000e);
        // entl_device_t *entl_dev = adapter->entl_dev;
        // entl_state_machine_t *stm = entl_dev->edev_stm; // or mcn (within entl_state_machine.c)
        // (dynamic) entl_mgr_t *mgr = entl_dev->edev_mgr
        // e.g. mgr->emf_event

        // ecnl private data (after shared data, 'mgr_plus.emp_base')
        entl_mgr_plus_t *mgr_plus = kzalloc(sizeof(struct entl_mgr_plus), GFP_ATOMIC);
        if (!mgr_plus) continue; // ENOMEM;

        mgr_plus->emp_plug_in = plug_in; // ecnl instance
        mgr_plus->emp_hack = hack;

        // backdoor into encl (for e1000e/entl use)
        entl_mgr_t *mgr = (entl_mgr_t *) mgr_plus; // i.e. mgr_plus.emp_base, callback struct
        mgr->emf_event = adapt_event;
        mgr->emf_private = mgr;

        int magic = ENCL_ENTL_MAGIC;
        int compat = funcs->edf_validate(e1000e, magic, mgr); // ref: entl_device.c
        if (compat < 0) {
            PLUG_DEBUG(plug_in, "incompatible i/f 0x%x \"%s\" ??", magic, hack->name);
            continue;
        }

        int port_id = ecnl_register_port(module_id, hack->name, hack->e1000e, funcs);
        if (port_id < 0) { PLUG_DEBUG(plug_in, "failed to register \"%s\"", hack->name); continue; }

        // Q : how do we figure out port_id from e1000e ref ??
        // NOT : int port_id = hack->index;
        hack->port_id = port_id;

        // data structures from the ecnl plug-in side:
        // struct net_device *plug_in
        // ecnl_device_t *e_dev = (ecnl_device_t *) netdev_priv(plug_in);
        // struct entl_driver *e_driver = &e_dev->ecnl_drivers[port_id];
        // e.g. e_driver->funcs : edf_validate, edf_get_state, edf_send_AIT, edf_retrieve_AIT w/eda_device (e1000e)
    }
}
    
// Q: what locking is required here?
// /Users/bjackson/git-projects/ubuntu-bionic/include/linux/netdevice.h
extern struct net init_net;
static void scan_netdev(struct net_device *plug_in) {
    read_lock(&dev_base_lock);
    const struct net *net = &init_net;
    struct net_device *n_dev; for_each_netdev(net, n_dev) {
    // for (struct net_device *n_dev = first_net_device(net); n_dev; n_dev = next_net_device(n_dev)) {
        PLUG_DEBUG(plug_in, "scan_netdev considering [%s]", n_dev->name);
        inject_dev(n_dev);
    }
    read_unlock(&dev_base_lock);
}

// --

static void ecnl_setup(struct net_device *plug_in) {
    plug_in->netdev_ops = &ecnl_netdev_ops;
    plug_in->flags |= IFF_NOARP;
}

static int __init ecnl_init_module(void) {
    if (device_busy) {
        DEFMOD_DEBUG("ecnl_init_module - called on busy state");
        return -EINVAL;
    }

    pr_info("Earth Computing Generic Netlink Module - %s\n", ECNL_DEVICE_DRIVER_VERSION);
    pr_info("Copyright(c) 2018, 2019 Earth Computing\n");

#ifndef BIONIC
    int err = genl_register_family_with_ops_groups(&nl_ecnd_fam, nl_ecnl_ops, nl_ecnd_mcgrps);
#else
    int err = genl_register_family(&nl_ecnd_fam); // , nl_ecnl_ops, nl_ecnd_mcgrps);
#endif
    if (err) {
        DEFMOD_DEBUG("ecnl_init_module - failed register genetlink family: \"%s\"", nl_ecnd_fam.name);
        return -EINVAL;
    }

    DEFMOD_DEBUG("registered genetlink family: \"%s\"", nl_ecnd_fam.name);

    struct net_device *plug_in = alloc_netdev(sizeof(ecnl_device_t), MAIN_DRIVER_NAME, NET_NAME_UNKNOWN, ecnl_setup);
    ecnl_device_t *this_device = (ecnl_device_t *) netdev_priv(plug_in);
    memset(this_device, 0, sizeof(ecnl_device_t));
    strcpy(this_device->ecnl_name, MAIN_DRIVER_NAME);
    this_device->ecnl_index = 0;

    spin_lock_init(&this_device->ecnl_lock);
    device_busy = 1;

    //inter_module_register("ecnl_driver_funcs", THIS_MODULE, ecnl_funcs);

    if (register_netdev(plug_in)) {
        DEFMOD_DEBUG("ecnl_init_module - failed register net_dev: \"%s\"", this_device->ecnl_name);
    }

    ecnl_devices[num_ecnl_devices++] = plug_in;

    scan_netdev(plug_in);
    hack_init(plug_in);
    return 0;
}

// FIXME: clean up data
static void __exit ecnl_cleanup_module(void) {
    if (device_busy) {
        DEFMOD_DEBUG("ecnl_cleanup_module - busy");
        //inter_module_unregister("ecnl_driver_funcs");
        device_busy = 0;
    }
    else {
        DEFMOD_DEBUG("ecnl_cleanup_module - non-busy");
    }
}

module_init(ecnl_init_module);
module_exit(ecnl_cleanup_module);

MODULE_LICENSE("GPL");
MODULE_ALIAS_RTNL_LINK(DRV_NAME);
MODULE_VERSION(DRV_VERSION);

