/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
#include "ecnl_proto.h"

int doit(struct nl_sock *sock, struct nl_msg *msg) {
    uint32_t module_id = 0;
    uint32_t actual_module_id;

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

    uint32_t port_id = 0;
    uint32_t actual_port_id = 0;

// num_ports from get_module_info (above)
for (uint32_t port_id = 0; port_id < num_ports; port_id++) {
CLEAR_MSG;
    {
printf("get_port_state %d\n", port_id);
        link_state_t link_state; memset(&link_state, 0, sizeof(link_state_t));
        int rc = get_port_state(sock, msg, module_id, port_id, &actual_module_id, &actual_port_id, &link_state);
        if (rc < 0) fatal_error(rc, "get_port_state");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
    }
}

// --

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

    struct nl_sock *sock = init_sock();
printf("init_sock\n");

    char *nlctrl = "nlctrl";
    if (genl_ctrl_resolve(sock, nlctrl) != GENL_ID_CTRL) {
        fatal_error(NLE_INVAL, "Resolving of \"%s\" failed", nlctrl);
    }

printf("genl_ctrl_resolve(nlctrl)\n");

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
