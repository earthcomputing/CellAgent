#ifndef ECNL_ENTL_IF_H
#define ECNL_ENTL_IF_H

typedef struct ec_state {
    uint64_t ecs_recover_count;
    uint64_t ecs_recovered_count;
    uint64_t ecs_s_count;
    uint64_t ecs_r_count;
    uint64_t ecs_entt_count;
    uint64_t ecs_aop_count;
    int ecs_link_state;
    int ecs_num_queued;
    struct timespec ecs_update_time;
#ifdef ENTL_SPEED_CHECK
    struct timespec ecs_interval_time;
    struct timespec ecs_max_interval_time;
    struct timespec ecs_min_interval_time;
#endif
} ec_state_t;

// for now, allow jumbo packets
#define EC_MESSAGE_MAX 9000
typedef struct ec_ait_data {
    uint32_t ecad_message_len;
    char ecad_data[EC_MESSAGE_MAX];
    // uint32_t op_code;
} ec_ait_data_t;

typedef struct ec_alo_reg {
    uint32_t ecar_index;
    uint64_t ecar_reg;
} ec_alo_reg_t;

typedef struct ec_alo_regs {
    uint64_t ecars_regs[32];
    uint32_t ecars_flags;
} ec_alo_regs_t;

#define ENCL_ENTL_MAGIC 0x5affdead
typedef struct entl_driver_funcs {
    int (*edf_validate)           (struct net_device *dev, int magic);
    netdev_tx_t (*edf_start_xmit) (struct sk_buff *skb, struct net_device *dev);
    int (*edf_send_AIT)           (struct sk_buff *skb, struct net_device *dev);
    int (*edf_retrieve_AIT)       (struct net_device *dev, ec_ait_data_t *data);
    int (*edf_write_reg)          (struct net_device *dev, ec_alo_reg_t *reg);
    int (*edf_read_regset)        (struct net_device *dev, ec_alo_regs_t *regs);
    int (*edf_get_state)          (struct net_device *dev, ec_state_t *state);
} entl_driver_funcs_t;

#ifdef ENTL_ADAPT_IMPL
static int adapt_validate(struct net_device *dev, int magic); // { return 1; }
static netdev_tx_t adapt_start_xmit(struct sk_buff *skb, struct net_device *e1000e); // { return NETDEV_TX_BUSY; }
static int adapt_send_AIT(struct sk_buff *skb, struct net_device *e1000e); // { return -1; }
static int adapt_retrieve_AIT(struct net_device *e1000e, ec_ait_data_t *data); // { return -1; }
static int adapt_write_reg(struct net_device *e1000e, ec_alo_reg_t *reg); // { return -1; }
static int adapt_read_regset(struct net_device *e1000e, ec_alo_regs_t *regs); // { return -1; }
static int adapt_get_state(struct net_device *dev, ec_state_t *state); // { return -1; }

static entl_driver_funcs_t entl_adapt_funcs = {
    .edf_validate = &adapt_validate,
    .edf_start_xmit = &adapt_start_xmit,
    .edf_send_AIT = &adapt_send_AIT,
    .edf_retrieve_AIT = &adapt_retrieve_AIT,
    .edf_write_reg = &adapt_write_reg,
    .edf_read_regset = &adapt_read_regset,
    .edf_get_state = &adapt_get_state,
};

EXPORT_SYMBOL(entl_adapt_funcs);
#else
extern entl_driver_funcs_t entl_adapt_funcs;
#endif

#endif
