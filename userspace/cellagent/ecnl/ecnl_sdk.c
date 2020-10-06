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
#include <stdlib.h>
#include <string.h>
#include <syslog.h>
#include <time.h>
#include <unistd.h>
#include <net/if.h> // for struct ifreq
#include <sys/ioctl.h>
#include <sys/types.h>
extern void perror (const char *__s); //usr/include/stdio.h

#include "entl_ioctl.h"

int priority = LOG_DAEMON | LOG_INFO;
// #define SYSLOG(fmt, ...) printf(fmt "\n", ...)
#define SYSLOG(fmt, args...) syslog(priority, fmt "\n", ## args)

#ifdef BIONIC
#define NUM_INTERFACES 1
static char *fixed_names[] = { "eno1" };
#else
#define NUM_INTERFACES 4
static char *fixed_names[] = { "enp6s0", "enp7s0", "enp8s0", "enp9s0" };
#endif

// logical port(s):
int nchan; //  = NUM_INTERFACES;
char **port_name; // [NUM_INTERFACES];
static struct ifreq *entl_device; // [NUM_INTERFACES];
static struct entl_ioctl_data *ioctl_data; // [NUM_INTERFACES];

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

static link_device_t *links; // [NUM_INTERFACES];

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

static int err_sock;
typedef pthread_mutex_t mutex_t;
static mutex_t access_mutex;
#define ACCESS_LOCK pthread_mutex_lock(&access_mutex)
#define ACCESS_UNLOCK pthread_mutex_unlock(&access_mutex)

// has side-effect of clearing error
// entl_read_error_state() - memset(&mcn->error_state, 0, sizeof(entl_state_t));
static int read_error(struct ifreq *r) {
    ACCESS_LOCK;
    int rc = ioctl(err_sock, SIOCDEVPRIVATE_ENTL_RD_ERROR, r);
    ACCESS_UNLOCK;
    // if (!rc) share_data(r->ifr_data);
    return rc;
}

static int register_handler(struct ifreq *r) {
    ACCESS_LOCK;
    int rc = ioctl(err_sock, SIOCDEVPRIVATE_ENTL_SET_SIGRCVR, r);
    ACCESS_UNLOCK;
    return rc;
}


static void dump_data(char *name, struct entl_ioctl_data *q) {
    char *link = (q->link_state) ? "UP" : "DOWN"; ; // int, 0: down, 1: up
    int nqueue = q->num_queued; // uint32_t
    // q->state;
    entl_state_t *s = &q->state;
        uint32_t current_state = s->current_state; // 0:idle 1:H 2:W 3:S 4:R
        uint32_t seqno_recv = s->event_i_know;       // last event received
        uint32_t seqno_sent = s->event_i_sent;       // last event sent
        uint32_t seqno_next = s->event_send_next;    // next event sent
    // q->error_state;
    entl_state_t *err = &q->error_state;
        uint32_t flag = err->error_flag;         // first error
        uint32_t mask = err->p_error_flag;       // when multiple, union of error bits
        uint32_t count = err->error_count;        // multiple errors
        struct timespec first = err->error_time;  // first error detected (usec), struct timespec
        struct timespec recent = err->update_time; // last updated (usec), struct timespec
    // ENTL_SPEED_CHECK
        // interval_time; // duration between S <-> R transition
        // max_interval_time;
        // min_interval_time;

    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC_RAW, &ts); // maybe: CLOCK_REALTIME, ignoring errors
    if (flag != 0) {
        SYSLOG("%ld %s dump_data:"
            " link %s"
            " nqueue %d"
            " state %d"
            " seqno:"
            " _recv %d"
            " _sent %d"
            " _next %d"
            " error:"
            " flag 0x%04x"
            " mask 0x%04x"
            " count %d",
            ts.tv_sec, name,
            link,
            nqueue,
            current_state,
            seqno_recv,
            seqno_sent,
            seqno_next,
            flag,
            mask,
            count
        );
    }
    else {
        SYSLOG("%ld %s dump_data:"
            " link %s"
            " nqueue %d"
            " state %d"
            " seqno:"
            " _recv %d"
            " _sent %d"
            " _next %d",
            ts.tv_sec, name,
            link,
            nqueue,
            current_state,
            seqno_recv,
            seqno_sent,
            seqno_next
        );
    }
}

// FIXME: this is where "route repair" would happen
static void service_device(struct ifreq *r) {
    // SYSLOG("%s: service_device\n", r->ifr_name);
    int rc = read_error(r);
    if (rc == -1) { SYSLOG("%s: service_device - read_error failed", r->ifr_name); return; }

    struct entl_ioctl_data *q = (void *) r->ifr_data;
    dump_data(r->ifr_name, q);
}

// FIXME: we don't know which entl_device instance (port/link) sent us a signal ??
static void error_handler(int signum) {
    if (SIGUSR1 != signum) return;
    for (int i = 0; i < nchan; i++) {
        struct ifreq *r = &entl_device[i];
        service_device(r);
    }
}

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

// For error correction
  if (fork() == 0) {
    if ((err_sock = socket(AF_INET, SOCK_DGRAM, 0)) < 0) { perror("socket"); exit(-1); }

    port_name = fixed_names;
    nchan = NUM_INTERFACES;

    entl_device = calloc(sizeof(struct ifreq), nchan); if (!entl_device) { perror("calloc"); exit(-1); }
    ioctl_data = calloc(sizeof(struct entl_ioctl_data), nchan); if (!ioctl_data) { perror("calloc"); exit(-1); }
    // links = calloc(sizeof(link_device_t), nchan); if (!links) { perror("calloc"); exit(-1); }

    int nochdir = 0; // cwd root
    int noclose = 0; // close 0/1/2
    if (daemon(nochdir, noclose) < 0) { perror("daemon"); exit(-1); }

    const char *name = "clear_error";
    const char *base = strrchr(name, '/');
    const char *ident = (base) ? &base[1] : name;
    int option = LOG_PID;
    int facility = LOG_DAEMON | LOG_INFO;
    openlog(ident, option, facility);

    SYSLOG("starting ...");

#if 0
    // initialize data structure - links
    for (int i = 0; i < nchan; i++) {
        char *n = port_name[i];
        link_device_t *link = &links[i];
        init_link(link, n);
    }

    // share initial state
    // FIXME: redundant w/get initial state unless read_error failed ??
    for (int i = 0; i < nchan; i++) {
        link_device_t *link = &links[i];
        int len = toJSON(link);
        toServer(link->json);
    }
#endif

    // initialize data structure - ioctl_data
    int pid = getpid();
    for (int i = 0; i < nchan; i++) {
        struct entl_ioctl_data *q = &ioctl_data[i];
        memset(q, 0, sizeof(struct entl_ioctl_data));
        q->pid = pid;
    }

    // initialize data structure - entl_device
    for (int i = 0; i < nchan; i++) {
        char *n = port_name[i];
        struct entl_ioctl_data *q = &ioctl_data[i];
        struct ifreq *r = &entl_device[i];
        memset(r, 0, sizeof(struct ifreq));
        r->ifr_data = (void *) q; // FIXME: what type is it?
        strncpy(r->ifr_name, n, sizeof(r->ifr_name)); // #define IF_NAMESIZE 16
    }

    // set handler
    signal(SIGUSR1, error_handler);

    // register for signal(s)
    for (int i = 0; i < nchan; i++) {
        struct ifreq *r = &entl_device[i];
        int rc = register_handler(r);
        if (rc == -1) { SYSLOG("%s: register_handler failed", r->ifr_name); exit(-1); }
        SYSLOG("%s start", r->ifr_name);
    }

    // get initial state (may have missed last signal)
    for (int i = 0; i < nchan; i++) {
        struct ifreq *r = &entl_device[i];
        service_device(r);
        // int rc = read_error(r);
        // if (rc == -1) { SYSLOG("%s: read_error failed", r->ifr_name); exit(-1); }
    }
    while (1) {
        sleep(1);
    }

    // NOTREACHED
    closelog();
    return 0; // normal exit
  }
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

