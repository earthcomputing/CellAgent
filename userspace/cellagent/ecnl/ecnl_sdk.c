#include <netlink/attr.h>
#include <netlink/cli/utils.h>
#include <linux/genetlink.h>


#include "ecnl_sdk.h"

typedef struct {
  struct nl_sock *sock;
  uint32_t module_id;
} nl_session_t;

struct nl_sock *init_sock();

// For error correction
#include <errno.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <syslog.h>
#include <time.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <sys/types.h>
extern void perror (const char *__s); //usr/include/stdio.h

#include "entl_ioctl.h"

#if 0
typedef struct {
    char *name; // unused
    int linkState;
    int entlState;
    int entlCount;
    char AITMessageR[256]; // unused
    char AITMessageS[256]; // unused
    char json[512];
} link_device_t;

static void init_link(link_device_t *link, char *n) {
    memset(link, 0, sizeof(link_device_t));
    link->name = n;
    link->entlState = 100; // unknown
    sprintf(link->AITMessageS, " ");
    sprintf(link->AITMessageR, " ");
}

static void share_data(struct entl_ioctl_data *q) {
    link_device_t p;
    p.entlState = q->state.current_state;
    p.entlCount = q->state.event_i_know;
    p.linkState = q->link_state;
    int len = toJSON(&p);
    toServer(p.json);
}
#endif

// Interface routines
int alloc_nl_session(void **nl_session_ptr) {
    int err;
    *nl_session_ptr = (nl_session_t *) malloc(sizeof(nl_session_t));
    nl_session_t *nl_session = *((nl_session_t **) nl_session_ptr);
    nl_session->sock = init_sock();
    printf("init_sock\n");
    struct nl_sock *sock = nl_session->sock;
    char *nlctrl = "nlctrl";
    if (genl_ctrl_resolve(sock, nlctrl) != GENL_ID_CTRL) {
        fatal_error(NLE_INVAL, "Resolving of \"%s\" failed", nlctrl);
    }
    printf("genl_ctrl_resolve(nlctrl)\n");

    // Comment out either this or setting of static module_id.  If using this, make module_id non-const.
    // char *module_name = "sim_ecnl0";
    // int rc = alloc_driver(nl_session->sock, nl_session->msg, module_name, &(nl_session->module_id));
    // if (rc < 0) fatal_error(rc, "alloc_driver");
    nl_session->module_id = 0;
};

int free_nl_session(void *nl_session_void) {
    nl_session_t *nl_session = ((nl_session_t *) nl_session_void);
    struct nl_sock *sock = nl_session->sock;
    nl_close(sock);
    nl_socket_free(sock);
    free(nl_session);
};


int ecnl_get_module_info(void *nl_session_void, const module_info_t **mipp) {
    nl_session_t *nl_session = ((nl_session_t *) nl_session_void);
    struct nl_sock *sock = nl_session->sock;
    struct nl_msg *msg = nlmsg_alloc();
    if (msg == NULL) {
        fatal_error(NLE_NOMEM, "Unable to allocate netlink message");
    }
    uint32_t module_id = nl_session->module_id;
    *mipp = malloc(sizeof(module_info_t));
    if (*mipp != NULL) {
        // memset(*mipp, 0, sizeof(module_info_t));
        module_info_t *settable_mip = (module_info_t *)(*mipp);
	int rc = get_module_info(sock, msg, module_id, settable_mip);
	if (rc < 0) fatal_error(rc, "get_module_info");
	nlmsg_free(msg);
	printf("module_id: %d\n", settable_mip->module_id);
	printf("module_name: %s\n", settable_mip->module_name);
	printf("num_ports: %d\n", settable_mip->num_ports);
	fflush(stdout);
	return 0;
    }
    nlmsg_free(msg);
    fatal_error(-1, "module_info allocation");
}


static int ecnl_alloc_driver(struct nl_sock *sock, char *module_name, uint32_t *module_id_p) {
    struct nl_msg *msg = nlmsg_alloc();
    int rc = alloc_driver(sock, msg, module_name, module_id_p);
    if (rc < 0) fatal_error(rc, "alloc_driver");
    nlmsg_free(msg);
    return 0;
}

int ecnl_alloc_table(void *nl_session_void, uint32_t table_size, const uint32_t **table_id_p) {
    nl_session_t *nl_session = ((nl_session_t *) nl_session_void);
    struct nl_sock *sock = nl_session->sock;
    struct nl_msg *msg = nlmsg_alloc();
    uint32_t module_id = nl_session->module_id;
    uint32_t actual_module_id;
    *table_id_p = malloc(sizeof(uint32_t));
    if (*table_id_p != NULL) {
        // memset(*table_idp, 0, sizeof(uint32_t));
        uint32_t *settable_table_id_p = (uint32_t *)(*table_id_p);
	int rc = alloc_table(sock, msg, module_id, table_size, &actual_module_id, settable_table_id_p);
        if (rc < 0) fatal_error(rc, "alloc_table");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
	nlmsg_free(msg);
	return 0;
    }
    nlmsg_free(msg);
    fatal_error(-1, "table allocation");
}

int ecnl_dealloc_table(void *nl_session_void, uint32_t table_id) {
    nl_session_t *nl_session = ((nl_session_t *) nl_session_void);
    struct nl_sock *sock = nl_session->sock;
    struct nl_msg *msg = nlmsg_alloc();
    uint32_t module_id = nl_session->module_id;
    uint32_t actual_module_id;
    uint32_t actual_table_id;
    int rc = dealloc_table(sock, msg, module_id, table_id, &actual_module_id, &actual_table_id);
    if (rc < 0) fatal_error(rc, "dealloc_table");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_table_id != table_id) fatal_error(-1, "table mismatch: %d, %d", table_id, actual_table_id);
    nlmsg_free(msg);
    return 0;
}

int ecnl_select_table(void *nl_session_void, uint32_t table_id) {
    nl_session_t *nl_session = ((nl_session_t *) nl_session_void);
    struct nl_sock *sock = nl_session->sock;
    struct nl_msg *msg = nlmsg_alloc();
    uint32_t module_id = nl_session->module_id;
    uint32_t actual_module_id;
    uint32_t actual_table_id;
    int rc = select_table(sock, msg, module_id, table_id, &actual_module_id, &actual_table_id);
    if (rc < 0) fatal_error(rc, "select_table");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_table_id != table_id) fatal_error(-1, "table mismatch: %d, %d", table_id, actual_table_id);
    nlmsg_free(msg);
    return 0;
}

int ecnl_fill_table(void *nl_session_void, uint32_t table_id, uint32_t table_location, uint32_t table_content_size, ecnl_table_entry_t *table_content) {
    nl_session_t *nl_session = ((nl_session_t *) nl_session_void);
    struct nl_sock *sock = nl_session->sock;
    struct nl_msg *msg = nlmsg_alloc();
    uint32_t module_id = nl_session->module_id;
    uint32_t actual_module_id;
    uint32_t actual_table_id;
    int rc = fill_table(sock, msg, module_id, table_id, table_location, table_content_size, table_content,  &actual_module_id, &actual_table_id);
    if (rc < 0) fatal_error(rc, "fill_table");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_table_id != table_id) fatal_error(-1, "table mismatch: %d, %d", table_id, actual_table_id);
    nlmsg_free(msg);
    return 0;
}

int ecnl_fill_table_entry(void *nl_session_void, uint32_t table_id, uint32_t table_location, ecnl_table_entry_t *table_entry) {
    nl_session_t *nl_session = ((nl_session_t *) nl_session_void);
    struct nl_sock *sock = nl_session->sock;
    struct nl_msg *msg = nlmsg_alloc();
    uint32_t module_id = nl_session->module_id;
    uint32_t actual_module_id;
    uint32_t actual_table_id;
    int rc = fill_table_entry(sock, msg, module_id, table_id, table_location, table_entry,  &actual_module_id, &actual_table_id);
    if (rc < 0) fatal_error(rc, "fill table entry");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    if (actual_table_id != table_id) fatal_error(-1, "table mismatch: %d, %d", table_id, actual_table_id);
    nlmsg_free(msg);
    return 0;
}

int ecnl_map_ports(void *nl_session_void, uint32_t **table_map_p) {
    nl_session_t *nl_session = ((nl_session_t *) nl_session_void);
    struct nl_sock *sock = nl_session->sock;
    struct nl_msg *msg = nlmsg_alloc();
    uint32_t module_id = nl_session->module_id;
    uint32_t actual_module_id;
    *table_map_p = malloc(sizeof(uint32_t));
    if (*table_map_p != NULL) {
        // memset(*table_map_p, 0, sizeof(uint32_t));
        uint32_t *settable_table_map = (uint32_t *)(*table_map_p);
	int rc = map_ports(sock, msg, module_id, settable_table_map, &actual_module_id);
        if (rc < 0) fatal_error(rc, "map ports");
        if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
	nlmsg_free(msg);
	return 0;
    }
    nlmsg_free(msg);
    fatal_error(-1, "table allocation");
}

int ecnl_start_forwarding(void *nl_session_void) {
    nl_session_t *nl_session = ((nl_session_t *) nl_session_void);
    struct nl_sock *sock = nl_session->sock;
    struct nl_msg *msg = nlmsg_alloc();
    uint32_t module_id = nl_session->module_id;
    uint32_t actual_module_id;
    int rc = start_forwarding(sock, msg, module_id, &actual_module_id);
    if (rc < 0) fatal_error(rc, "start forwarding");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    nlmsg_free(msg);
    return 0;
}

int ecnl_stop_forwarding(void *nl_session_void) {
    nl_session_t *nl_session = ((nl_session_t *) nl_session_void);
    struct nl_sock *sock = nl_session->sock;
    struct nl_msg *msg = nlmsg_alloc();
    uint32_t module_id = nl_session->module_id;
    uint32_t actual_module_id;
    int rc = stop_forwarding(sock, msg, module_id, &actual_module_id);
    if (rc < 0) fatal_error(rc, "stop forwarding");
    if (actual_module_id != module_id) fatal_error(-1, "module mismatch: %d, %d", module_id, actual_module_id);
    nlmsg_free(msg);
    return 0;
}

