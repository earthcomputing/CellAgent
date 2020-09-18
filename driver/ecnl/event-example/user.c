#include <netlink/netlink.h>
#include <netlink/socket.h>
#include <netlink/msg.h>
#include <netlink/genl/genl.h>
#include <netlink/genl/ctrl.h>

enum attributes {
    ATTR_DUMMY,
    ATTR_HELLO,
    ATTR_FOO,
    /* This must be last! */
    __ATTR_MAX,
};

enum commands {
    COMMAND_HELLO,
    /* This must be last! */
    __COMMAND_MAX,
};

static int fail(int error, char *func_name) {
    printf("%s() failed.\n", func_name);
    return error;
}

static int nl_fail(int error, char *func_name) {
    printf("%s (%d)\n", nl_geterror(error), error);
    return fail(error, func_name);
}

static int cb(struct nl_msg *msg, void *arg) {
    struct nlmsghdr *nl_hdr = nlmsg_hdr(msg);
    struct genlmsghdr *genl_hdr = genlmsg_hdr(nl_hdr);
    if (genl_hdr->cmd != COMMAND_HELLO) { printf("bad message type: %d\n", genl_hdr->cmd); return 0; }

    struct nlattr *attrs[__ATTR_MAX];
    int error = genlmsg_parse(nl_hdr, 0, attrs, __ATTR_MAX - 1, NULL);
    if (error) return nl_fail(error, "genlmsg_parse");

    struct nlattr *ap;
    ap = attrs[ATTR_HELLO];
    if (ap) printf("ATTR_HELLO: len:%u type:%u data:%s\n", ap->nla_len, ap->nla_type, (char *)nla_data(ap));
    ap = attrs[ATTR_FOO];
    if (ap) printf("ATTR_FOO: len:%u type:%u data:%u\n", ap->nla_len, ap->nla_type, *((__u32 *)nla_data(ap)));
    return 0;
}

#define FAMILY_NAME "PotatoFamily"
#define GROUP_NAME "PotatoGroup"

/* register with multicast group*/
static int do_listen(struct nl_sock *sk, char *family, char *group_name) {
    int group = genl_ctrl_resolve_grp(sk, family, group_name);
    if (group < 0) { printf(FAMILY_NAME " - genl_ctrl_resolve_grp (%s) failed: %s", group_name, nl_geterror(group)); return group; }
    printf(FAMILY_NAME " - group %s (%d)", group_name, group);
    int error = nl_socket_add_memberships(sk, group, 0);
    if (error) { printf(FAMILY_NAME " - nl_socket_add_memberships failed: %d", error); return error; }
    return error;
}

static struct nl_sock *sk = NULL;

static int do_things(void) {
    sk = nl_socket_alloc();
    if (!sk) return fail(-1, "nl_socket_alloc");
    nl_socket_disable_seq_check(sk);
    int error = nl_socket_modify_cb(sk, NL_CB_VALID, NL_CB_CUSTOM, cb, NULL);
    if (error) return nl_fail(error, "nl_socket_modify_cb");
    error = genl_connect(sk);
    if (error) return nl_fail(error, "genl_connect");

    error = do_listen(sk, FAMILY_NAME, GROUP_NAME);
    if (error) { printf("do_listen() failed: %d\n", error); return error; }

    nl_recvmsgs_default(sk);
    return 0;
}

int main(void) {
    int error = do_things();
    if (sk) nl_socket_free(sk);
    return error;
}
