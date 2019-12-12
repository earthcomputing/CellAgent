// retrieve_ait_message
// send_ait_message
// get_module_info

#include <unistd.h>
#include "ecnl_proto.h"
#include "ecnl_endpoint.h"

// context sensitive (ept)
int ept_verbose = 1;
#define EPT_DEBUG(fmt, args...) if (ept_verbose) { printf("%s (%d) " fmt "\n", ept->ept_name, ept->ept_port_id, ## args); } else { }

// --

static char *special = "\f\n\r\t\v"; // np nl cr ht vt \a bel \b bs

// with thanks to the Remington No. 2 (1878):
// 07 bel 08 bs 09 ht 0a nl 0b vt 0c np 0d cr
static int non_printf(unsigned char ch) {
    if (ch > 0x7e) return 1; // DEL or not 7-bit
    if (ch > 0x1f) return 0; // DEL or not 7-bit
    if (!strchr(special, ch)) return 1;
    return 0;
}

static int scanbuf(unsigned char *buf, int len) {
    for (int i = 0; i < len - 1; i++) {
        unsigned char ch = buf[i];
        int is_unprintable = non_printf(ch);
        if (is_unprintable) return 0;
    }
    if (buf[len - 1] != '\0') return 0;
    return 1;
}

// --

static void module_info(struct nl_sock *sock, module_info_t *mi) {
    int module_id = 0;
    struct nl_msg *msg = nlmsg_alloc();
    memset(mi, 0, sizeof(module_info_t));
    int rc = get_module_info(sock, msg, module_id, mi);
    if (rc < 0) fatal_error(rc, "get_module_info");
    nlmsg_free(msg);
}

static void get_link_state(ecnl_endpoint_t *ept, link_state_t *link_state) {
    uint32_t actual_module_id;
    uint32_t actual_port_id = 0;
    struct nl_msg *msg = nlmsg_alloc();
    memset(link_state, 0, sizeof(link_state_t));
    int rc = get_port_state((struct nl_sock *) (ept->ept_sock), msg, ept->ept_module_id, ept->ept_port_id, &actual_module_id, &actual_port_id, link_state);
    if (rc < 0) fatal_error(rc, "get_port_state");
    if (actual_module_id != ept->ept_module_id) fatal_error(-1, "module mismatch: %d, %d", ept->ept_module_id, actual_module_id);
    if (actual_port_id != ept->ept_port_id) fatal_error(-1, "port mismatch: %d, %d", ept->ept_port_id, actual_port_id);
    nlmsg_free(msg);
}

// --

extern void ept_do_read_async(ecnl_endpoint_t *ept, ept_buf_desc_t *actual_buf) {
    // FIXME: how do we know buffer length?
    memset(actual_buf, 0, sizeof(ept_buf_desc_t));
    alo_reg_t alo_reg = { .ar_no = 0, .ar_data = 0, };
    uint32_t actual_module_id;
    uint32_t actual_port_id = 0;
    struct nl_msg *msg = nlmsg_alloc();
    int rc = retrieve_ait_message((struct nl_sock *) (ept->ept_sock), msg, ept->ept_module_id, ept->ept_port_id, alo_reg, &actual_module_id, &actual_port_id, (buf_desc_t *) actual_buf); // ICK cast.
    if (rc < 0) fatal_error(rc, "retrieve_ait_message");
    if (actual_module_id != ept->ept_module_id) fatal_error(-1, "module mismatch: %d, %d", ept->ept_module_id, actual_module_id);
    if (actual_port_id != ept->ept_port_id) fatal_error(-1, "port mismatch: %d, %d", ept->ept_port_id, actual_port_id);
    nlmsg_free(msg);
    EPT_DEBUG("async: (len %d)", actual_buf->len);
}

extern void ept_dumpbuf(ecnl_endpoint_t *ept, char *tag, ept_buf_desc_t *buf) {
    // no data
    if ((buf->len < 1) || (!buf->frame)) {
        EPT_DEBUG("retr: (empty %d)", buf->len);
        return;
    }

    int asciz = scanbuf((unsigned char *) buf->frame, buf->len);
    char *flavor = (asciz) ? "asciz" : "blob";
    EPT_DEBUG("%s (%s %d) - '%s'", tag, flavor, buf->len, (asciz) ? (char *) buf->frame : "");
}

extern void ept_do_read(ecnl_endpoint_t *ept, ept_buf_desc_t *actual_buf, int nsecs) {
    // memset(actual_buf, 0, sizeof(ept_buf_desc_t));
    for (int i = 0; i < nsecs; i++) {
        ept_do_read_async(ept, actual_buf);
        if ((actual_buf->len < 1) || (!actual_buf->frame)) {
            sleep(1);
            continue;
        }
        break;
    }

    ept_dumpbuf(ept, "ept_do_read", actual_buf);
}

extern void ept_do_xmit(ecnl_endpoint_t *ept, ept_buf_desc_t *buf) {
    uint32_t actual_module_id;
    uint32_t actual_port_id = 0;
    struct nl_msg *msg = nlmsg_alloc();

    ept_dumpbuf(ept, "ept_do_xmit", buf);

    int rc = send_ait_message((struct nl_sock *) (ept->ept_sock), msg, ept->ept_module_id, ept->ept_port_id, *(buf_desc_t *) buf, &actual_module_id, &actual_port_id); // ICK cast.
    if (rc < 0) fatal_error(rc, "send_ait_message");
    if (actual_module_id != ept->ept_module_id) fatal_error(-1, "module mismatch: %d, %d", ept->ept_module_id, actual_module_id);
    if (actual_port_id != ept->ept_port_id) fatal_error(-1, "port mismatch: %d, %d", ept->ept_port_id, actual_port_id);
    nlmsg_free(msg);
}

extern void ept_update(ecnl_endpoint_t *ept) {
    link_state_t link_state; 
    get_link_state(ept, &link_state);
    ept->ept_up_down = link_state.port_link_state;
}

// FIXME: what's a "struct ept_event" look like ??
extern void ept_get_event(ecnl_endpoint_t *ept, ecnl_event_t *ep) {
    uint32_t actual_module_id;
    uint32_t actual_port_id = 0;
    int cmd_id;
    uint32_t num_ait_messages;
    link_state_t link_state; 
    read_event((struct nl_sock *) (ept->ept_esock), &actual_module_id, &actual_port_id, &cmd_id, &num_ait_messages, &link_state);
    EPT_DEBUG("event: module_id %d port_id %d", actual_module_id, actual_port_id);


    // meant for this endpoint?
    if (actual_port_id == ept->ept_port_id) {
        char *up_down = (link_state.port_link_state) ? "UP" : "DOWN";
        EPT_DEBUG("event: cmd_id %d n_msg %d link %s", cmd_id, num_ait_messages, up_down);
    }

    ep->event_module_id = actual_module_id;
    ep->event_port_id = actual_port_id;
    ep->event_cmd_id = cmd_id;
    ep->event_n_msgs = num_ait_messages;
    ep->event_up_down = link_state.port_link_state;
}

extern int ecnl_init(bool debug) {
    if (!debug) ecp_verbose = 0;
    // if (!debug) ept_verbose = 0;
    struct nl_sock *sock = init_sock();
    module_info_t mi;
    module_info(sock, &mi);
    nl_close(sock);
    nl_socket_free(sock);
    uint32_t num_ports = mi.num_ports;
    return num_ports;
}

// per-endpoint sock
extern ecnl_endpoint_t *ept_create(uint32_t port_id) {
    struct nl_sock *sock = init_sock();
    struct nl_sock *esock = init_sock_events();
    ecnl_endpoint_t *ept = malloc(sizeof(ecnl_endpoint_t)); memset(ept, 0, sizeof(ecnl_endpoint_t));
    ept->ept_sock = sock;
    ept->ept_esock = esock;
    ept->ept_module_id = 0; // hardwired
    ept->ept_port_id = port_id;

    link_state_t link_state; 
    get_link_state(ept, &link_state);
    ept->ept_up_down = link_state.port_link_state;
    ept->ept_name = link_state.port_name; // fill in name
}

extern void ept_destroy(ecnl_endpoint_t *ept) {
    nl_close((struct nl_sock *) (ept->ept_sock));
    nl_socket_free((struct nl_sock *) (ept->ept_sock));
}

// --

#if 0
#ifndef BIONIC
int def_send_port_id = 3; // enp7s0
int def_retr_port_id = 2; // enp9s0
#else
int def_send_port_id = 0; // enp6s0 or eno1
int def_retr_port_id = 0; // enp6s0 or eno1
#endif

int main(int argc, char *argv[]) {
    uint32_t num_ports = ecnl_init();
    for (uint32_t port_id = 0; port_id < num_ports; port_id++) {
        ecnl_endpoint_t *ept = ept_create(port_id);
    }
}
#endif
