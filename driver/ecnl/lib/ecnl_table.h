#ifndef _ECNL_TABLE_H_
#define _ECNL_TABLE_H_

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

#endif
