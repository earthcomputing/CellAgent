/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
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
    printf(ECNL_GENL_NAME " my_func" "\n");
    SYSLOG(ECNL_GENL_NAME " - event");
    return 0;
}

// #define NL_ECNL_MULTICAST_GOUP_LINKSTATUS "status"
// #define NL_ECNL_MULTICAST_GOUP_AIT "ait"
// #define NL_ECNL_MULTICAST_GOUP_ALO "alo"
// #define NL_ECNL_MULTICAST_GOUP_DISCOVERY "discovery"
// #define NL_ECNL_MULTICAST_GOUP_TEST "test"

char *GROUPS[] = { NL_ECNL_MULTICAST_GOUP_LINKSTATUS, NL_ECNL_MULTICAST_GOUP_AIT };

/* register with multicast group*/
static int do_listen(struct nl_sock *sk, char *family, char *group_name) {
    int group = genl_ctrl_resolve_grp(sk, family, group_name);
    if (group < 0) { SYSLOG(ECNL_GENL_NAME " - genl_ctrl_resolve_grp (%s) failed: %s", group_name, nl_geterror(group)); return group; }
    SYSLOG(ECNL_GENL_NAME " - group %s (%d)", group_name, group);
    int error = nl_socket_add_memberships(sk, group, 0);
    if (error) { SYSLOG(ECNL_GENL_NAME " - nl_socket_add_memberships failed: %d", error); return error; }
    return error;
}

void forever(void) {
    struct nl_sock *sk = init_sock_route();
    nl_socket_modify_cb(sk, NL_CB_VALID, NL_CB_CUSTOM, my_func, NULL);
    int rc = genl_ctrl_resolve(sk, ECNL_GENL_NAME);
    if (rc < 0) { perror("genl_ctrl_resolve"); return; }

    for (int i = 0; i < ARRAY_SIZE(GROUPS); i++) {
        char *group_name = GROUPS[i];
        do_listen(sk, ECNL_GENL_NAME, group_name);
    }

    SYSLOG(ECNL_GENL_NAME " - listening ...");
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

    SYSLOG(ECNL_GENL_NAME " - starting ...");

    int nochdir = 0; // cwd root
    int noclose = 0; // close 0/1/2
    // if (daemon(nochdir, noclose) < 0) { perror("daemon"); exit(-1); }
    // syslog(LOG_DAEMON | LOG_ERR, "daemon: %s", strerror(errno));
    // ugh - fatal_error()

    forever();
    return 0;
}
