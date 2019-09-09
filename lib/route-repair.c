// route-repair

#include <errno.h>
#include <net/if.h> // for struct ifreq
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/types.h>
#include <unistd.h>
extern void perror (const char *__s); //usr/include/stdio.h

#include "entl_ioctl.h"


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

// FIXME: this is where "route repair" would happen
static void service_device(struct ifreq *r) {
    int rc = read_error(r);
    if (rc == -1) { printf("%s: read_error failed\n", r->ifr_name); return; }
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
    if ((sock = socket(AF_INET, SOCK_DGRAM, 0)) < 0) { perror("socket"); return 0; }

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
        if (rc == -1) { printf("%s: register_handler failed\n", r->ifr_name); return -1; } // exit
    }

    // get initial state (may have missed last signal)
    for (int i = 0; i < NUM_INTERFACES; i++) {
        struct ifreq *r = &entl_device[i];
        service_device(r);
        // int rc = read_error(r);
        // if (rc == -1) { printf("%s: read_error failed\n", r->ifr_name); return -1; } // exit
    }

    while (1) {
        sleep(1);
    }

    return 0;
}
