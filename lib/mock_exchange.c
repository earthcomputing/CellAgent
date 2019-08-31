#include "ecnl_proto.h"

#define CLEAR_MSG { nlmsg_free(msg); msg = nlmsg_alloc(); }

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
        printf("\n");
    }

    {
        CLEAR_MSG;
        printf("map_ports\n");
        uint32_t table_map[] = { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14 }; // sizeof(u32) * ENCL_FW_TABLE_ENTRY_ARRAY
        int rc = map_ports(sock, msg, module_id, table_map, &actual_module_id);
        if (rc < 0) fatal_error(rc, "map_ports");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    }

    {
        char FRAME[] = "Plain Text Message";
        buf.len = strlen(FRAME) + 1; // include NUL
        buf.frame = (uint8_t *) FRAME;

        CLEAR_MSG;
        printf("send_ait_message\n");
        uint32_t message_length;
        uint8_t *frame;
        int rc = send_ait_message(sock, msg, module_id, send_port_id, buf, &actual_module_id, &actual_port_id);
        if (rc < 0) fatal_error(rc, "send_ait_message");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != send_port_id) fatal_error(-1, "port mismatch: %d, %d", send_port_id, actual_port_id);
    }

    {
        CLEAR_MSG;
        printf("retrieve_ait_message\n");
        buf_desc_t actual_buf; memset(&actual_buf, 0, sizeof(buf_desc_t));
        int rc = retrieve_ait_message(sock, msg, module_id, retr_port_id, alo_reg, &actual_module_id, &actual_port_id, &actual_buf);
        if (rc < 0) fatal_error(rc, "retrieve_ait_message");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != retr_port_id) fatal_error(-1, "port mismatch: %d, %d", retr_port_id, actual_port_id);

        printf("retr: %d '%s'\n", actual_buf.len, (char *) actual_buf.frame); // assumes c-string
    }
}

int main(int argc, char *argv[]) {
    int err;

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
