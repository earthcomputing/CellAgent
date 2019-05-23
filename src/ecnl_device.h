#ifndef _ECNL_DEVICE_H_
#define _ECNL_DEVICE_H_

#define ECNL_DEBUG(fmt, args...) printk(KERN_ALERT "ECNL:" fmt, ## args)

typedef struct entl_driver {
    unsigned char *eda_name;
    int eda_index;
    struct net_device *eda_device;
    entl_driver_funcs_t *eda_funcs; // entl_adapt_funcs
} entl_driver_t;

#define ENCL_FW_TABLE_ENTRY_ARRAY 15
typedef struct ecnl_table_entry {
    union {
        uint32_t raw_vector;
        struct {
            unsigned int reserved : 12;
            unsigned int parent : 4;
            unsigned int port_vector : 16;
        };
    } info;
    uint32_t nextID[ENCL_FW_TABLE_ENTRY_ARRAY];
} ecnl_table_entry_t;

#define ECNL_NAME_LEN 20
#define ENTL_TABLE_MAX 16
#define ECNL_FW_TABLE_ENTRY_SIZE (sizeof(uint32_t) * (ENCL_FW_TABLE_ENTRY_ARRAY + 1))
#define ENTL_DRIVER_MAX 16
typedef struct ecnl_device {
    unsigned char ecnl_name[ECNL_NAME_LEN];
    int ecnl_index;
    ecnl_table_entry_t *ecnl_current_table;
    uint32_t ecnl_current_table_size;
    ecnl_table_entry_t *ecnl_tables[ENTL_TABLE_MAX];
    uint32_t ecnl_tables_size[ENTL_TABLE_MAX];
    bool ecnl_fw_enable;
    uint32_t ecnl_fw_map[ENCL_FW_TABLE_ENTRY_ARRAY];
    struct net_device_stats ecnl_net_stats;
    spinlock_t ecnl_lock;
    u16 ecnl_num_ports;
    entl_driver_t ecnl_drivers[ENTL_DRIVER_MAX];
} ecnl_device_t;

#define ECNL_DRIVER_MAX 1024
#define ECNL_FW_TABLE_VECTOR_MASK 0xFFFF
#define ECNL_TABLE_NUM_ENTRIES ((ECNL_TABLE_WORD_SIZE * 8) / ECNL_TABLE_BIT_SIZE)
#define ENTL_NAME_MAX_LEN 80;
#define MAIN_DRIVER_NAME "ecnl0"

// interface function table to lower drivers
struct ecnl_funcs {
    int (*ecnlf_register_port)(int encl_id, unsigned char *name, struct net_device *device, entl_driver_funcs_t *funcs);
    int (*ecnlf_receive_skb)(int encl_id, int drv_index, struct sk_buff *skb);
    //int (*ecnlf_receive_dsc)(int encl_id, int drv_index, struct sk_buff *skb);
    void (*ecnlf_link_status_update)(int encl_id, int drv_index, struct ec_state *state);
    void (*ecnlf_forward_ait_message)(int encl_id, int drv_index, struct sk_buff *skb);
    void (*ecnlf_got_ait_massage)(int encl_id, int drv_index, int num_message);
    void (*ecnlf_got_alo_update)(int encl_id, int drv_index);
    void (*ecnlf_deregister_ports)(int encl_id);
} ecnl_funcs_t;

#endif
