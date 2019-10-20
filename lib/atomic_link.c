// listen.c

#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <syslog.h>
#include <time.h>
#include <unistd.h>
#include <sys/types.h>
extern void perror (const char *__s); //usr/include/stdio.h

int priority = LOG_DAEMON | LOG_INFO;
// #define SYSLOG(fmt, ...) printf(fmt "\n", ...)
#define SYSLOG(fmt, args...) syslog(priority, fmt "\n", ## args)

#include "ecnl_proto.h"
// #include "ecnl_protocol.h" // enum nl_ecnd_multicast_groups;

extern struct nl_sock *init_sock_route() {
    struct nl_sock *sock = nl_socket_alloc();
    nl_connect(sock, NETLINK_GENERIC); // NETLINK_ROUTE);
    nl_socket_disable_seq_check(sock);
    return sock;
}

static int my_func(struct nl_msg *msg, void *arg) {
    printf("my_funct" "\n");
    return 0;
}

void forever(void) {
    struct nl_sock *sk = init_sock_route();
    nl_socket_modify_cb(sk, NL_CB_VALID, NL_CB_CUSTOM, my_func, NULL);
    // nl_socket_add_memberships(sk, RTNLGRP_LINK, 0);
#if 1
    nl_socket_add_memberships(sk, NL_ECNL_MCGRP_LINKSTATUS, NL_ECNL_MCGRP_AIT, NL_ECNL_MCGRP_ALO, NL_ECNL_MCGRP_DISCOVERY, NL_ECNL_MCGRP_TEST, 0);
#endif

    while (1) {
        nl_recvmsgs_default(sk);
    }
}

int main(int argc, char *argv[]) {
    const char *base = strrchr(argv[0], '/');
    const char *ident = (base) ? &base[1] : argv[0];
    int option = LOG_PID;
    int facility = LOG_DAEMON | LOG_INFO;
    openlog(ident, option, facility);

    SYSLOG("starting ...");

    int nochdir = 0; // cwd root
    int noclose = 0; // close 0/1/2
    // if (daemon(nochdir, noclose) < 0) { perror("daemon"); exit(-1); }
    // syslog(LOG_DAEMON | LOG_ERR, "daemon: %s", strerror(errno));
    // ugh - fatal_error()

    forever();
}
