#include <netlink/attr.h>
#include <netlink/cli/utils.h>
#include <linux/genetlink.h>


#include "ecnl_sdk.h"

#define CLEAR_MSG { nlmsg_free(msg); msg = nlmsg_alloc(); }

typedef struct {
  struct nl_sock *sock;
  struct nl_msg *msg;
  uint32_t module_id;
} nl_session_t;

typedef nl_session_t ecnl_session_t;

struct nl_sock *init_sock();

int alloc_ecnl_session(void **ecnl_session_ptr) {
    int err;
    *ecnl_session_ptr = (ecnl_session_t *) malloc(sizeof(ecnl_session_t));
    ecnl_session_t *ecnl_session = *((ecnl_session_t **) ecnl_session_ptr);
    ecnl_session->sock = init_sock();
    printf("init_sock\n");
    struct nl_sock *sock = ecnl_session->sock;
    char *nlctrl = "nlctrl";
    if (genl_ctrl_resolve(sock, nlctrl) != GENL_ID_CTRL) {
        fatal_error(NLE_INVAL, "Resolving of \"%s\" failed", nlctrl);
    }
    printf("genl_ctrl_resolve(nlctrl)\n");

    ecnl_session->msg = nlmsg_alloc();
    struct nl_msg *msg = ecnl_session->msg;
    if (msg == NULL) {
        fatal_error(NLE_NOMEM, "Unable to allocate netlink message");
    }

    // Comment out either this or setting of static module_id.  If using this, make module_id non-const.
    // char *module_name = "sim_ecnl0";
    // int rc = alloc_driver(ecnl_session->sock, ecnl_session->msg, module_name, &(ecnl_session->module_id));
    // if (rc < 0) fatal_error(rc, "alloc_driver");
    ecnl_session->module_id = 0;
};

int free_ecnl_session(void *ecnl_session) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    nlmsg_free(msg);
    nl_close(sock);
    nl_socket_free(sock);
    free((ecnl_session_t *) ecnl_session);
};


int ecnl_get_module_info(void *ecnl_session, const module_info_t **mipp) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    *mipp = malloc(sizeof(module_info_t));
    if (*mipp != NULL) {
        // memset(*mipp, 0, sizeof(module_info_t));
        module_info_t *settable_mip = (module_info_t *)(*mipp);
	int rc = get_module_info(sock, msg, module_id, settable_mip);
	if (rc < 0) fatal_error(rc, "get_module_info");
	return 0;
    }
    fatal_error(-1, "module_info allocation");
}


int ecnl_get_port_state(void *ecnl_session, uint32_t port_id, const link_state_t **lspp) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    uint32_t actual_port_id;
    *lspp = malloc(sizeof(link_state_t));
    if (*lspp != NULL) {
        // memset(*lspp, 0, sizeof(link_state_t));
        link_state_t *settable_lsp = (link_state_t *)(*lspp);
        int rc = get_port_state(sock, msg, module_id, port_id, &actual_module_id, &actual_port_id, settable_lsp);
        if (rc < 0) fatal_error(rc, "get_port_state");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
        // printf("Link is %s - '%s' (%d)\n", (*lspp)->port_link_state) ? "Up" : "Down", (*lspp)->port_name, port_id);
	return 0;
    }
    fatal_error(-1, "link_state allocation");
}


static int ecnl_alloc_driver(struct nl_sock *sock, struct nl_msg *msg, char *module_name, uint32_t *module_id_p) {
    CLEAR_MSG;
    int rc = alloc_driver(sock, msg, module_name, module_id_p);
    if (rc < 0) fatal_error(rc, "alloc_driver");
    return 0;
}

int ecnl_alloc_table(void *ecnl_session, uint32_t table_size, const uint32_t **table_id_p) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    *table_id_p = malloc(sizeof(uint32_t));
    if (*table_id_p != NULL) {
        // memset(*table_idp, 0, sizeof(uint32_t));
        uint32_t *settable_table_id_p = (uint32_t *)(*table_id_p);
	int rc = alloc_table(sock, msg, module_id, table_size, &actual_module_id, settable_table_id_p);
        if (rc < 0) fatal_error(rc, "alloc_table");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
	return 0;
    }
    fatal_error(-1, "table allocation");
}

int ecnl_dealloc_table(void *ecnl_session, uint32_t table_id) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    uint32_t actual_table_id;
    int rc = dealloc_table(sock, msg, module_id, table_id, &actual_module_id, &actual_table_id);
    if (rc < 0) fatal_error(rc, "dealloc_table");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_table_id != table_id) fatal_error(-1, "table mismatch: %d, %d", table_id, actual_table_id);
    return 0;
}

int ecnl_select_table(void *ecnl_session, uint32_t table_id) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    uint32_t actual_table_id;
    int rc = select_table(sock, msg, module_id, table_id, &actual_module_id, &actual_table_id);
    if (rc < 0) fatal_error(rc, "select_table");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_table_id != table_id) fatal_error(-1, "table mismatch: %d, %d", table_id, actual_table_id);
    return 0;
}

int ecnl_fill_table(void *ecnl_session, uint32_t table_id, uint32_t table_location, uint32_t table_content_size, ecnl_table_entry_t *table_content) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    uint32_t actual_table_id;
    int rc = fill_table(sock, msg, module_id, table_id, table_location, table_content_size, table_content,  &actual_module_id, &actual_table_id);
    if (rc < 0) fatal_error(rc, "fill_table");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_table_id != table_id) fatal_error(-1, "table mismatch: %d, %d", table_id, actual_table_id);
    return 0;
}

int ecnl_fill_table_entry(void *ecnl_session, uint32_t table_id, uint32_t table_location, ecnl_table_entry_t *table_entry) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    uint32_t actual_table_id;
    int rc = fill_table_entry(sock, msg, module_id, table_id, table_location, table_entry,  &actual_module_id, &actual_table_id);
    if (rc < 0) fatal_error(rc, "fill table entry");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_table_id != table_id) fatal_error(-1, "table mismatch: %d, %d", table_id, actual_table_id);
    return 0;
}

int ecnl_map_ports(void *ecnl_session, uint32_t **table_map_p) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    *table_map_p = malloc(sizeof(uint32_t));
    if (*table_map_p != NULL) {
        // memset(*table_map_p, 0, sizeof(uint32_t));
        uint32_t *settable_table_map = (uint32_t *)(*table_map_p);
	int rc = map_ports(sock, msg, module_id, settable_table_map, &actual_module_id);
        if (rc < 0) fatal_error(rc, "map ports");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
	return 0;
    }
    fatal_error(-1, "table allocation");
}

int ecnl_start_forwarding(void *ecnl_session) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    int rc = start_forwarding(sock, msg, module_id, &actual_module_id);
    if (rc < 0) fatal_error(rc, "start forwarding");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    return 0;
}

int ecnl_stop_forwarding(void *ecnl_session) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    int rc = stop_forwarding(sock, msg, module_id, &actual_module_id);
    if (rc < 0) fatal_error(rc, "stop forwarding");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    return 0;
}

int ecnl_read_alo_register(void *ecnl_session, uint32_t port_id, uint32_t alo_reg_no, uint64_t *alo_reg_data_p) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    uint32_t actual_port_id;
    alo_reg_t settable_alo_reg;
    settable_alo_reg.ar_no = alo_reg_no;
    uint32_t *fp = NULL; // FIX ME
    uint64_t **vp = NULL; // FIX ME
    // NOT DEFINED TO TAKE A POINTER FOR alo_reg
    // WHAT's fp & vp?
    // THIS IS WRONG -- MUST TAKE &settable_alo_reg
    int rc = read_alo_registers(sock, msg, module_id, port_id, settable_alo_reg, &actual_module_id, &actual_port_id, fp, vp);
    if (rc < 0) fatal_error(rc, "read alo register");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
    *alo_reg_data_p = settable_alo_reg.ar_data;
    return rc;
}

int ecnl_retrieve_ait_message(void *ecnl_session, uint32_t port_id, buf_desc_t **buf_desc_pp) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    uint32_t actual_port_id;
    alo_reg_t alo_reg;
    *buf_desc_pp = malloc(sizeof(buf_desc_t));
    if (*buf_desc_pp != NULL) {
        // memset(*buf_desc_pp, 0, sizeof(buf_desc_t));
        buf_desc_t *settable_buf_desc_p = (buf_desc_t *)(*buf_desc_pp);
        int rc = retrieve_ait_message(sock, msg, module_id, port_id, alo_reg, &actual_module_id, &actual_port_id, settable_buf_desc_p);
        if (rc < 0) fatal_error(rc, "retrieve_ait_message");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
        if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
        // printf("Received %d bytes from port %d\n", (*buf_desc_pp)->len, port_id);
	return 0;
    }
    fatal_error(-1, "link_state allocation");
}

int ecnl_write_alo_register(void *ecnl_session, uint32_t port_id, uint32_t alo_reg_no, uint64_t alo_reg_data) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    uint32_t actual_port_id;
    alo_reg_t alo_reg;
    alo_reg.ar_no = alo_reg_no;
    alo_reg.ar_data = alo_reg_data;
    int rc = write_alo_register(sock, msg, module_id, port_id, alo_reg, &actual_module_id, &actual_port_id);
    if (rc < 0) fatal_error(rc, "write_alo_register");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
    return 0;
}

int ecnl_send_ait_message(void *ecnl_session, uint32_t port_id, buf_desc_t buf_desc) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    uint32_t actual_port_id;
    int rc = send_ait_message(sock, msg, module_id, port_id, buf_desc, &actual_module_id, &actual_port_id);
    if (rc < 0) fatal_error(rc, "send_ait_message");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
    return 0;
}

// fire-and-forget (i.e. no response)

// SEND_DISCOVER_MESSAGE(uint32_t module_id, uint32_t port_id, uint32_t message_length, uint8_t *message)
int ecnl_send_discover_message(void *ecnl_session, uint32_t port_id, buf_desc_t buf_desc) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    // This one doesn't take mp, pp?
    int rc = send_discover_message(sock, msg, module_id, port_id, buf_desc);
    if (rc < 0) fatal_error(rc, "send_discover_message");
    return 0;
}

// --

// dummy func ?? (aka send_ait_message)

int ecnl_signal_ait_message(void *ecnl_session, uint32_t port_id, buf_desc_t buf_desc) {
    struct nl_sock *sock = ((ecnl_session_t *) ecnl_session)->sock;
    struct nl_msg *msg = ((ecnl_session_t *) ecnl_session)->msg;
    CLEAR_MSG;
    uint32_t module_id = ((ecnl_session_t *) ecnl_session)->module_id;
    uint32_t actual_module_id;
    uint32_t actual_port_id;
    int rc = signal_ait_message(sock, msg, module_id, port_id, buf_desc, &actual_module_id, &actual_port_id);
    if (rc < 0) fatal_error(rc, "send_ait_message");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_port_id != port_id) fatal_error(-1, "port mismatch: %d, %d", port_id, actual_port_id);
    return 0;
}
