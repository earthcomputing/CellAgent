#include <unistd.h>
#include "ecnl_proto.h"

#define CLEAR_MSG { nlmsg_free(msg); msg = nlmsg_alloc(); }

typedef struct {
    char *name;
    uint32_t port_id;
    int up_down;;
} endpoint_t;

endpoint_t port_pair[2];

#ifndef BIONIC
int def_send_port_id = 3; // enp7s0
int def_retr_port_id = 2; // enp9s0
#else
int def_send_port_id = 0; // enp6s0 or eno1
int def_retr_port_id = 0; // enp6s0 or eno1
#endif

char *special = "\f\n\r\t\v"; // np nl cr ht vt \a bel \b bs

// with thanks to the Remington No. 2 (1878):
// 07 bel 08 bs 09 ht 0a nl 0b vt 0c np 0d cr
int non_printf(unsigned char ch) {
    if (ch > 0x7e) return 1; // DEL or not 7-bit
    if (ch > 0x1f) return 0; // DEL or not 7-bit
    if (!strchr(special, ch)) return 1;
    return 0;
}

int scanbuf(unsigned char *buf, int len) {
    for (int i = 0; i < len - 1; i++) {
        unsigned char ch = buf[i];
        int is_unprintable = non_printf(ch);
        if (is_unprintable) return 0;
    }
    if (buf[len - 1] != '\0') return 0;
    return 1;
}

int do_read_async(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, endpoint_t *ept, buf_desc_t *actual_buf) {
    alo_reg_t alo_reg = {
        .ar_no = 0,
        .ar_data = 0,
    };

    uint32_t actual_module_id;
    uint32_t actual_port_id = 0;

    CLEAR_MSG;
    printf("retrieve_ait_message %d (%s)\n", ept->port_id, ept->name);
    int rc = retrieve_ait_message(sock, msg, module_id, ept->port_id, alo_reg, &actual_module_id, &actual_port_id, actual_buf);
    if (rc < 0) fatal_error(rc, "retrieve_ait_message");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_port_id != ept->port_id) fatal_error(-1, "port mismatch: %d, %d", ept->port_id, actual_port_id);
}

void do_read(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, endpoint_t *ept, int nsecs) {
    buf_desc_t actual_buf; memset(&actual_buf, 0, sizeof(buf_desc_t));
    // actual_buf.frame = xx; // who's responsible for buffer mgmt ??
    // at the moment, the serialization layer 'adapts'; 
    // this implies client is responsible for freeing buffers
    // and needs to determine if lower layer re-allocated the buf

    for (int i = 0; i < nsecs; i++) {
        do_read_async(sock, msg, module_id, ept, &actual_buf);

        if ((actual_buf.len < 1) || (!actual_buf.frame)) {
            // printf("retr: NO DATA ??\n");
            sleep(1);
            continue;
        }
        break;
    }

    int asciz = scanbuf((unsigned char *) actual_buf.frame, actual_buf.len);
    if (asciz) {
        printf("retr: (asciz %d) '%s'\n", actual_buf.len, (char *) actual_buf.frame); // assumes c-string
    }
    else {
        printf("retr: (blob %d)\n", actual_buf.len); // dump?
    }

    printf("\n");
}

void do_xmit(struct nl_sock *sock, struct nl_msg *msg, uint32_t module_id, endpoint_t *ept, buf_desc_t buf) {
    uint32_t actual_module_id;
    uint32_t actual_port_id = 0;

    CLEAR_MSG;

    int asciz = scanbuf((unsigned char *) buf.frame, buf.len);
    char *tag = (asciz) ? "asciz" : "blob";
    printf("send_ait_message (%s %d) %d (%s) - '%s'\n", tag, buf.len, ept->port_id, ept->name, (asciz) ? (char *) buf.frame : "");
    int rc = send_ait_message(sock, msg, module_id, ept->port_id, buf, &actual_module_id, &actual_port_id);
    if (rc < 0) fatal_error(rc, "send_ait_message");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_port_id != ept->port_id) fatal_error(-1, "port mismatch: %d, %d", ept->port_id, actual_port_id);
}

int doit(struct nl_sock *sock, struct nl_msg *msg) {
    uint32_t module_id = 0;
    uint32_t actual_module_id;

    uint32_t actual_port_id = 0;
    uint32_t num_ports = -1;

    alo_reg_t alo_reg = {
        .ar_no = 0,
        .ar_data = 0,
    };

    // determine num_ports
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

        // associate names with (port) numbers
        for (int i = 0; i < 2; i++) {
            endpoint_t *p = &port_pair[i];
            if (!p->name && (p->port_id == port_id)) {
                p->name = link_state.port_name; // fill in name
                p->up_down = link_state.port_link_state;
                continue;
            }
            if (p->name && (strcmp(link_state.port_name, p->name) == 0)) {
                p->port_id = port_id; // determine port_id
                p->up_down = link_state.port_link_state;
            }
        }
    }

    printf("\n");
    printf("send: %s (%d) %s\n", port_pair[0].name, port_pair[0].port_id, (port_pair[0].up_down) ? "Up" : "Down");
    printf("recv: %s (%d) %s\n", port_pair[1].name, port_pair[1].port_id, (port_pair[1].up_down) ? "Up" : "Down");
    printf("\n");

// VALIDATION CODE:

    char asciz_FRAME[] = "Plain Text Message"; // 506c61696e2054657874204d65737361676500
    buf_desc_t asciz_buf = {
        .len = strlen(asciz_FRAME) + 1, // include NUL
        .frame = (uint8_t *) asciz_FRAME
    };

    // extra test - full binary buffer
    // char ecad_data[EC_MESSAGE_MAX]; // 9000
    uint16_t blob_FRAME[9000 / 2]; for (int i = 0; i < 9000 / 2; i++) { blob_FRAME[i] = i; } // might want: i | 0x8080 ?
    buf_desc_t blob_buf = {
        .len = 1500 + 26, // MTU + ethernet header
        .frame = (uint8_t *) blob_FRAME
    };

    // should replace these variables:
    endpoint_t *master_ept = &port_pair[0];
    endpoint_t *slave_ept = &port_pair[1];

    do_xmit(sock, msg, module_id, master_ept, asciz_buf);
    do_xmit(sock, msg, module_id, master_ept, blob_buf);

    // in reverse:
    do_xmit(sock, msg, module_id, slave_ept, asciz_buf);
    do_xmit(sock, msg, module_id, slave_ept, blob_buf);

    do_read(sock, msg, module_id, slave_ept, 60);
    do_read(sock, msg, module_id, slave_ept, 60);

    // in reverse:
    do_read(sock, msg, module_id, master_ept, 60);
    do_read(sock, msg, module_id, master_ept, 60);
}

// e.g. usage: mock_exchange enp7s0 enp9s0
int main(int argc, char *argv[]) {
    port_pair[0].port_id = def_send_port_id;
    port_pair[1].port_id = def_retr_port_id;

    if (argc > 1) {
        port_pair[0].name = argv[1];
        port_pair[1].name = argv[1];
    }

    if (argc > 2) {
        port_pair[1].name = argv[2];
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
