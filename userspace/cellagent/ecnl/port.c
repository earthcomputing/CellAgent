/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
// retrieve_ait_message
// send_ait_message
// get_module_info

#include <unistd.h>
#include "ecnl_proto.h"
#include "port.h"

// context sensitive (port)
int port_verbose = 1;
#define PORT_DEBUG(fmt, args...) if (port_verbose) { printf("%s (%d) " fmt "\n", port->port_name, port->port_id, ## args); } else { }

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

static void get_link_state(ecnl_port_t *port, link_state_t *link_state) {
    uint32_t actual_module_id;
    uint32_t actual_port_id = 0;
    struct nl_msg *msg = nlmsg_alloc();
    memset(link_state, 0, sizeof(link_state_t));
    int rc = get_port_state((struct nl_sock *) (port->port_sock), msg, port->port_module_id, port->port_id, &actual_module_id, &actual_port_id, link_state);
    if (rc < 0) fatal_error(rc, "get_port_state");
    if (actual_module_id != port->port_module_id) fatal_error(-1, "module mismatch: %d, %d", port->port_module_id, actual_module_id);
    if (actual_port_id != port->port_id) fatal_error(-1, "port mismatch: %d, %d", port->port_id, actual_port_id);
    nlmsg_free(msg);
}

// --

extern void port_do_read_async(ecnl_port_t *port, port_buf_desc_t *bdp) {
    // FIXME: how do we know buffer length?
    uint32_t actual_module_id;
    uint32_t actual_port_id = 0;
    struct nl_msg *msg = nlmsg_alloc();

    alo_reg_t alo_reg = { .ar_no = 0, .ar_data = 0, };
    // memset(bdp, 0, sizeof(buf_desc_t));
    int rc = retrieve_ait_message((struct nl_sock *) (port->port_sock), msg, port->port_module_id, port->port_id, alo_reg, &actual_module_id, &actual_port_id, (buf_desc_t *) bdp);
    if (rc < 0) fatal_error(rc, "retrieve_ait_message");
    if (actual_module_id != port->port_module_id) fatal_error(-1, "module mismatch: %d, %d", port->port_module_id, actual_module_id);
    if (actual_port_id != port->port_id) fatal_error(-1, "port mismatch: %d, %d", port->port_id, actual_port_id);
    nlmsg_free(msg);
    PORT_DEBUG("async: (len %d)", bdp->len);
}

extern void port_dumpbuf(ecnl_port_t *port, char *tag, const port_buf_desc_t *bdp) {
    // no data
    if ((bdp->len < 1) || (!bdp->frame)) {
        PORT_DEBUG("retr: (empty %d)", bdp->len);
        return;
    }

    int asciz = scanbuf((unsigned char *) bdp->frame, bdp->len);
    char *flavor = (asciz) ? "asciz" : "blob";
    PORT_DEBUG("%s (%s %d) - '%s'", tag, flavor, bdp->len, (asciz) ? (char *) bdp->frame : "");
}

extern void port_do_read(ecnl_port_t *port, port_buf_desc_t *bdp, int nsecs) {
    // memset(bdp, 0, sizeof(port_buf_desc_t));
    for (int i = 0; i < nsecs; i++) {
        port_do_read_async(port, bdp);
        if ((bdp->len < 1) || (!bdp->frame)) {
            sleep(1);
            continue;
        }
        break;
    }

    port_dumpbuf(port, "port_do_read", bdp);
}

extern void port_do_xmit(ecnl_port_t *port, const port_buf_desc_t *bdp) {
    uint32_t actual_module_id;
    uint32_t actual_port_id = 0;
    struct nl_msg *msg = nlmsg_alloc();

    port_dumpbuf(port, "port_do_xmit", bdp);

    int rc = send_ait_message((struct nl_sock *) (port->port_sock), msg, port->port_module_id, port->port_id, *(buf_desc_t *) bdp, &actual_module_id, &actual_port_id); // ICK cast.
    if (rc < 0) fatal_error(rc, "send_ait_message");
    if (actual_module_id != port->port_module_id) fatal_error(-1, "module mismatch: %d, %d", port->port_module_id, actual_module_id);
    if (actual_port_id != port->port_id) fatal_error(-1, "port mismatch: %d, %d", port->port_id, actual_port_id);
    nlmsg_free(msg);
}

void port_read_alo_register(ecnl_port_t *port, uint32_t alo_reg_no, uint64_t *alo_reg_data_p) {
    uint32_t actual_module_id;
    uint32_t actual_port_id = 0;
    struct nl_msg *msg = nlmsg_alloc();

    alo_reg_t alo_reg;
    alo_reg.ar_no = alo_reg_no;
    uint32_t *fp = NULL; // FIX ME
    uint64_t **vp = NULL; // FIX ME
    // NOT DEFINED TO TAKE A POINTER FOR alo_reg
    // WHAT's fp & vp?
    // THIS IS WRONG -- MUST TAKE &alo_reg
    int rc = read_alo_registers((struct nl_sock *) (port->port_sock), msg, port->port_module_id, port->port_id, alo_reg, &actual_module_id, &actual_port_id, fp, vp);
    if (rc < 0) fatal_error(rc, "read alo register");
    if (actual_module_id != port->port_module_id) fatal_error(-1, "module mismatch: %d, %d", port->port_module_id, actual_module_id);
    if (actual_port_id != port->port_id) fatal_error(-1, "port mismatch: %d, %d", port->port_id, actual_port_id);
    *alo_reg_data_p = alo_reg.ar_data;
    nlmsg_free(msg);
}

void port_write_alo_register(ecnl_port_t *port, uint32_t alo_reg_no, uint64_t alo_reg_data) {
    uint32_t actual_module_id;
    uint32_t actual_port_id = 0;
    struct nl_msg *msg = nlmsg_alloc();

    alo_reg_t alo_reg;
    alo_reg.ar_no = alo_reg_no;
    alo_reg.ar_data = alo_reg_data;
    int rc = write_alo_register((struct nl_sock *) (port->port_sock), msg, port->port_module_id, port->port_id, alo_reg, &actual_module_id, &actual_port_id);
    if (rc < 0) fatal_error(rc, "write_alo_register");
    if (actual_module_id != port->port_module_id) fatal_error(-1, "module mismatch: %d, %d", port->port_module_id, actual_module_id);
    if (actual_port_id != port->port_id) fatal_error(-1, "port mismatch: %d, %d", port->port_id, actual_port_id);
    nlmsg_free(msg);
}

extern void port_update(ecnl_port_t *port) {
    link_state_t link_state; 
    get_link_state(port, &link_state);
    port->port_up_down = link_state.port_link_state;
}

// FIXME: what's a "struct port_event" look like ??
extern void port_get_event(ecnl_port_t *port, ecnl_event_t *ep) {
    uint32_t actual_module_id;
    uint32_t actual_port_id = 0;
    int cmd_id;
    uint32_t num_ait_messages;
    link_state_t link_state;

    while (true) {
      read_event((struct nl_sock *) (port->port_esock), &actual_module_id, &actual_port_id, &cmd_id, &num_ait_messages, &link_state);

      // meant for this port?
      if (actual_port_id == port->port_id) {
        PORT_DEBUG("event: module_id %d port_id %d", actual_module_id, actual_port_id);
        char *up_down = (link_state.port_link_state) ? "UP" : "DOWN";
        PORT_DEBUG("event: cmd_id %d n_msg %d link %s", cmd_id, num_ait_messages, up_down);

	ep->event_module_id = actual_module_id;
	ep->event_port_id = actual_port_id;
	ep->event_cmd_id = cmd_id;
	ep->event_n_msgs = num_ait_messages;
	ep->event_up_down = link_state.port_link_state;
	break;
      }
    }
}

extern int ecnl_init(bool debug) {
    if (!debug) ecp_verbose = 0;
    // if (!debug) port_verbose = 0;
    struct nl_sock *sock = init_sock();
    module_info_t mi;
    module_info(sock, &mi);
    nl_close(sock);
    nl_socket_free(sock);
    uint32_t num_ports = mi.num_ports;
    return num_ports;
}

// per-port sock
extern ecnl_port_t *port_create(uint8_t port_id) {
    struct nl_sock *sock = init_sock();
    struct nl_sock *esock = init_sock_events();
    ecnl_port_t *ecnl_port_ptr = malloc(sizeof(ecnl_port_t));
    ecnl_port_ptr->port_sock = sock;
    ecnl_port_ptr->port_esock = esock;
    ecnl_port_ptr->port_module_id = 0; // hardwired
    ecnl_port_ptr->port_id = port_id;

    link_state_t link_state; 
    get_link_state(&ecnl_port, &link_state);
    ecnl_port_ptr->port_up_down = link_state.port_link_state;
    ecnl_port_ptr->port_name = link_state.port_name; // fill in name
    return ecnl_port_ptr;
}

extern void port_destroy(ecnl_port_t *port) {
    nl_close((struct nl_sock *) (port->port_sock));
    nl_socket_free((struct nl_sock *) (port->port_sock));
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
        ecnl_port_t *port = port_create(port_id);
    }
}

#endif
