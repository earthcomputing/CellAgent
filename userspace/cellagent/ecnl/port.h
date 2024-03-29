/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
#ifndef ECNL_PORT_H
#define ECNL_PORT_H

#include <unistd.h>
#include <stdbool.h>
#include <stdint.h>

// duplicates defn from ecnl_proto.h
typedef struct {
    uint32_t len;
    uint8_t *frame;
} port_buf_desc_t;

typedef struct {
    uint32_t port_module_id;
    void *port_sock; // struct nl_sock *
    void *port_esock; // struct nl_sock *
    char *port_name;
    uint8_t port_id;
    int port_up_down;
} ecnl_port_t;

extern int ecnl_init(bool debug);
// Returning struct because Rust doesn't want to store the pointers :-(.
extern ecnl_port_t *port_create(uint8_t port_id);
extern void port_destroy(ecnl_port_t *port);

extern void port_do_read_async(ecnl_port_t *port, port_buf_desc_t *bdp);
extern void port_do_read(ecnl_port_t *port, port_buf_desc_t *buf, int nsecs);
extern void port_do_xmit(ecnl_port_t *port, const port_buf_desc_t *buf);
extern void ecnl_read_alo_register(ecnl_port_t *port, uint32_t alo_reg_no, uint64_t *alo_reg_data_p);
extern void ecnl_write_alo_register(ecnl_port_t *port, uint32_t alo_reg_no, uint64_t alo_reg_data);
extern void port_update(ecnl_port_t *port);

typedef struct {
    uint32_t event_module_id;
    uint8_t event_port_id;
    int event_cmd_id;
    uint32_t event_n_msgs;
    int event_up_down;
} ecnl_event_t;

// events:
extern void port_get_event(ecnl_port_t *port, ecnl_event_t *eventp);

// debug:
extern void port_dumpbuf(ecnl_port_t *port, char *tag, const port_buf_desc_t *buf);

#endif
