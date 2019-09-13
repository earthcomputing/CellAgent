// route-repair
// man/3/daemon
// man/3/syslog

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
// #define SYSLOG(fmt, ...) printf(fmt, ...)
#define SYSLOG(fmt, args...) syslog(priority, fmt, ## args)

#ifdef BIONIC
#define NUM_INTERFACES 1
static char *port_name[NUM_INTERFACES] = { "eno1" };
#else
#define NUM_INTERFACES 4
static char *port_name[NUM_INTERFACES] = { "enp6s0", "enp7s0", "enp8s0", "enp9s0" };
#endif

// logical port(s):
static struct ifreq entl_device[NUM_INTERFACES];
static struct entl_ioctl_data ioctl_data[NUM_INTERFACES];

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

static link_device_t links[NUM_INTERFACES];

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

static int sock;

typedef pthread_mutex_t mutex_t;
static mutex_t access_mutex;
#define ACCESS_LOCK pthread_mutex_lock(&access_mutex)
#define ACCESS_UNLOCK pthread_mutex_unlock(&access_mutex)

// has side-effect of clearing error
// entl_read_error_state() - memset(&mcn->error_state, 0, sizeof(entl_state_t));
static int read_error(struct ifreq *r) {
    ACCESS_LOCK;
    int rc = ioctl(sock, SIOCDEVPRIVATE_ENTL_RD_ERROR, r);
    ACCESS_UNLOCK;
    // if (!rc) share_data(r->ifr_data);
    return rc;
}

static int register_handler(struct ifreq *r) {
    ACCESS_LOCK;
    int rc = ioctl(sock, SIOCDEVPRIVATE_ENTL_SET_SIGRCVR, r);
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
            " count %d"
            "\n",
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
            " _next %d"
            "\n",
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
    if (rc == -1) { SYSLOG("%s: service_device - read_error failed\n", r->ifr_name); return; }

    struct entl_ioctl_data *q = (void *) r->ifr_data;
    dump_data(r->ifr_name, q);
}

// FIXME: we don't know which entl_device instance (port/link) sent us a signal ??
static void error_handler(int signum) {
    if (SIGUSR1 != signum) return;
    for (int i = 0; i < NUM_INTERFACES; i++) {
        struct ifreq *r = &entl_device[i];
        service_device(r);
    }
}

// FIXME: could have instance list be CLI arguments
// that would allow for multiple listeners (donno if driver supports that?)
int main(int argc, char *argv[]) {
    if ((sock = socket(AF_INET, SOCK_DGRAM, 0)) < 0) { perror("socket"); exit(-1); }

    int nochdir = 0; // cwd root
    int noclose = 0; // close 0/1/2
    if (daemon(nochdir, noclose) < 0) { perror("daemon"); exit(-1); }

    const char *base = strrchr(argv[0], '/');
    const char *ident = (base) ? &base[1] : argv[0];
    int option = LOG_PID;
    int facility = LOG_DAEMON | LOG_INFO;
    openlog(ident, option, facility);

    SYSLOG("starting ...");

#if 0
    // initialize data structure - links
    for (int i = 0; i < NUM_INTERFACES; i++) {
        char *n = port_name[i];
        link_device_t *link = &links[i];
        init_link(link, n);
    }

    // share initial state
    // FIXME: redundant w/get initial state unless read_error failed ??
    for (int i = 0; i < NUM_INTERFACES; i++) {
        link_device_t *link = &links[i];
        int len = toJSON(link);
        toServer(link->json);
    }
#endif

    // initialize data structure - ioctl_data
    int pid = getpid();
    for (int i = 0; i < NUM_INTERFACES; i++) {
        struct entl_ioctl_data *q = &ioctl_data[i];
        memset(q, 0, sizeof(struct entl_ioctl_data));
        q->pid = pid;
    }

    // initialize data structure - entl_device
    for (int i = 0; i < NUM_INTERFACES; i++) {
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
    for (int i = 0; i < NUM_INTERFACES; i++) {
        struct ifreq *r = &entl_device[i];
        int rc = register_handler(r);
        if (rc == -1) { SYSLOG("%s: register_handler failed\n", r->ifr_name); exit(-1); }
    }

    // get initial state (may have missed last signal)
    for (int i = 0; i < NUM_INTERFACES; i++) {
        struct ifreq *r = &entl_device[i];
        service_device(r);
        // int rc = read_error(r);
        // if (rc == -1) { SYSLOG("%s: read_error failed\n", r->ifr_name); exit(-1); }
    }

    while (1) {
        sleep(1);
    }

    // NOTREACHED
    closelog();
    return 0; // normal exit
}
