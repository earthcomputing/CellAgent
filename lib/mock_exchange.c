#include <unistd.h>
#include "ecnl_proto.h"

#define CLEAR_MSG { nlmsg_free(msg); msg = nlmsg_alloc(); }

typedef struct {
    char *name;
    uint32_t port_id;
} endpoint_t;

endpoint_t port_pair[2];

#ifndef BIONIC
int send_port_id = 3; // enp7s0
int retr_port_id = 2; // enp9s0
#else
int send_port_id = 0; // enp6s0 or eno1
int retr_port_id = 0; // enp6s0 or eno1
#endif

int doit(struct nl_sock *sock, struct nl_msg *msg) {
    uint32_t module_id = 0;
    uint32_t actual_module_id;

    uint32_t port_id = 0;
    uint32_t actual_port_id = 0;
    uint32_t num_ports = -1;

    alo_reg_t alo_reg = {
        .ar_no = 0,
        .ar_data = 0,
    };

    buf_desc_t buf = {
        .len = 0,
        .frame = NULL,
    };

    {
        CLEAR_MSG;
        printf("get_module_info\n");
        module_info_t mi; memset(&mi, 0, sizeof(module_info_t));
        int rc = get_module_info(sock, msg, module_id, &mi);
        if (rc < 0) fatal_error(rc, "get_module_info");
        num_ports = mi.num_ports;
    }

    // num_ports from get_module_info (above)
    for (uint32_t port_id = 0; port_id < num_ports; port_id++) {
        CLEAR_MSG;
        printf("get_port_state %d\n", port_id);
        link_state_t link_state; memset(&link_state, 0, sizeof(link_state_t));
        int rc = get_port_state(sock, msg, module_id, port_id, &actual_module_id, &actual_port_id, &link_state);
        if (rc < 0) fatal_error(rc, "get_port_state");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);

        printf("Link is %s - '%s' (%d)\n", (link_state.port_link_state) ? "Up" : "Down", link_state.port_name, port_id);

        for (int i = 0; i < 2; i++) {
            endpoint_t *p = &port_pair[i];
            if (!p->name && p->port_id == port_id) {
                p->name = link_state.port_name; // fill in name
                continue;
            }
            if (p->name && strcmp(link_state.port_name, p->name)) {
                p->port_id = port_id; // determine port_id
            }
        }
        printf("\n");
    }

    // should replace these variables:
    send_port_id = port_pair[0].port_id;
    retr_port_id = port_pair[1].port_id;

    printf("send: %s (%d)\n", port_pair[0].name, send_port_id);
    printf("recv: %s (%d)\n", port_pair[1].name, retr_port_id);
    printf("\n");

    {
        CLEAR_MSG;
        printf("map_ports\n");
        uint32_t table_map[] = { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14 }; // sizeof(u32) * ENCL_FW_TABLE_ENTRY_ARRAY
        int rc = map_ports(sock, msg, module_id, table_map, &actual_module_id);
        if (rc < 0) fatal_error(rc, "map_ports");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    }

    {
        char FRAME[] = "Plain Text Message"; // 506c61696e2054657874204d65737361676500
        buf.len = strlen(FRAME) + 1; // include NUL
        buf.frame = (uint8_t *) FRAME;

        CLEAR_MSG;
        printf("send_ait_message (asciz %d) %d (%s)\n", buf.len, send_port_id, port_pair[0].name);
        int rc = send_ait_message(sock, msg, module_id, send_port_id, buf, &actual_module_id, &actual_port_id);
        if (rc < 0) fatal_error(rc, "send_ait_message");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != send_port_id) fatal_error(-1, "port mismatch: %d, %d", send_port_id, actual_port_id);
    }

sleep(1);

    {
        CLEAR_MSG;
        printf("retrieve_ait_message %d (%s)\n", retr_port_id, port_pair[1].name);
        buf_desc_t actual_buf; memset(&actual_buf, 0, sizeof(buf_desc_t));
        int rc = retrieve_ait_message(sock, msg, module_id, retr_port_id, alo_reg, &actual_module_id, &actual_port_id, &actual_buf);
        if (rc < 0) fatal_error(rc, "retrieve_ait_message");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != retr_port_id) fatal_error(-1, "port mismatch: %d, %d", retr_port_id, actual_port_id);

        printf("retr: (asciz %d) '%s'\n", actual_buf.len, (char *) actual_buf.frame); // assumes c-string
    }


    // extra test - full binary buffer
    // char ecad_data[EC_MESSAGE_MAX]; // 9000
    {
        uint16_t FRAME[9000 / 2]; for (int i = 0; i < 9000 / 2; i++) { FRAME[i] = i; } // might want: i | 0x8080 ?
        buf.len = 1500 + 26; // MTU + ethernet header
        buf.frame = (uint8_t *) FRAME;

        CLEAR_MSG;
        printf("send_ait_message (blob %d) %d (%s)\n", buf.len, send_port_id, port_pair[0].name);
        int rc = send_ait_message(sock, msg, module_id, send_port_id, buf, &actual_module_id, &actual_port_id);
        if (rc < 0) fatal_error(rc, "send_ait_message");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != send_port_id) fatal_error(-1, "port mismatch: %d, %d", send_port_id, actual_port_id);
    }

sleep(1);

    {
        CLEAR_MSG;
        printf("retrieve_ait_message %d (%s)\n", retr_port_id, port_pair[1].name);
        buf_desc_t actual_buf; memset(&actual_buf, 0, sizeof(buf_desc_t));
        int rc = retrieve_ait_message(sock, msg, module_id, retr_port_id, alo_reg, &actual_module_id, &actual_port_id, &actual_buf);
        if (rc < 0) fatal_error(rc, "retrieve_ait_message");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != retr_port_id) fatal_error(-1, "port mismatch: %d, %d", retr_port_id, actual_port_id);

        printf("retr: (blob %d)\n", actual_buf.len); // dump?
    }
}

// e.g. usage: mock_exchange enp7s0 enp9s0
int main(int argc, char *argv[]) {
    port_pair[0].port_id = send_port_id;
    port_pair[1].port_id = retr_port_id;

    if (argc > 1) {
        port_pair[0].name = argv[1];
        port_pair[1].name = argv[1];
        // printf("send: %s\n", port_pair[0].name);
    }

    if (argc > 2) {
        port_pair[1].name = argv[2];
        // printf("recv: %s\n", port_pair[1].name);
    }

    printf("init_sock\n");
    struct nl_sock *sock = init_sock();

    printf("nlmsg_alloc\n");
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
