/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
#include <linux/kernel.h>
#include <linux/module.h>
#include <net/genetlink.h>

static struct timer_list timer;

/**
 * runs whenever the socket receives messages.
 * unused, but Linux complains if we don't define it.
 */
static int hello(struct sk_buff *skb, struct genl_info *info) {
    pr_info("Received a message in kernelspace.\n");
    return 0;
}

enum attributes {
    /*
     * first is a throwaway empty attribute; I don't know why.
     * without it, ATTR_HELLO (the first one) stops working.
     */
    ATTR_DUMMY,
    ATTR_HELLO,
    ATTR_FOO,
    /* This must be last! */
    __ATTR_MAX,
};

static struct nla_policy policies[] = {
    [ATTR_HELLO] = { .type = NLA_STRING, },
    [ATTR_FOO] = { .type = NLA_U32, },
};

enum commands {
    COMMAND_HELLO,
    /* This must be last! */
    __COMMAND_MAX,
};

struct genl_ops ops[] = {
    {
        .cmd = COMMAND_HELLO,
        .flags = 0,
        .policy = policies,
        .doit = hello,
        .dumpit = NULL,
    },
};

struct genl_multicast_group groups[] = {
    { .name = "PotatoGroup" },
};

/**
 * A Generic Netlink family is a group of listeners who can and want to speak your language.
 * Anyone who wants to hear your messages needs to register to the same family as you.
 */
struct genl_family family = {
    .name = "PotatoFamily",
    .hdrsize = 0,
    .version = 1,
    .maxattr = __ATTR_MAX,
    // .netnsok = true,
    .ops = ops,
    .n_ops = ARRAY_SIZE(ops),
    .mcgrps = groups,
    .n_mcgrps = ARRAY_SIZE(groups),
};

static unsigned char *msg = "TEST";
static u32 foo = 12345;

/*
 * The family has only one group, so the group ID is just the family's group offset.
 * mcgrp_offset is supposed to be private, so use this value for debug purposes only.
 */
void send_multicast(struct timer_list *arg) {
    // pr_info("The group ID is %u.\n", family.mcgrp_offset);
    struct sk_buff *skb = genlmsg_new(NLMSG_GOODSIZE, GFP_KERNEL);
    if (!skb) { pr_err("genlmsg_new() failed.\n"); goto end; }
    void *msg_head = genlmsg_put(skb, 0, 0, &family, 0, COMMAND_HELLO);
    if (!msg_head) { pr_err("genlmsg_put() failed.\n"); kfree_skb(skb); goto end; }
    int error = nla_put_string(skb, ATTR_HELLO, msg);
    if (error) { pr_err("nla_put_string() failed: %d\n", error); kfree_skb(skb); goto end; }
    error = nla_put_u32(skb, ATTR_FOO, foo);
    if (error) { pr_err("nla_put_u32() failed: %d\n", error); kfree_skb(skb); goto end; }
    genlmsg_end(skb, msg_head);
    error = genlmsg_multicast_allns(&family, skb, 0, 0, GFP_KERNEL);
    if (error) { pr_err("genlmsg_multicast_allns() failed: %d\n", error); goto end; } // can happen when nobody is listening
    // should skb be freed ??
end:
    mod_timer(&timer, jiffies + msecs_to_jiffies(2000));
}

static void initialize_timer(void) {
#ifdef BIONIC
    timer_setup(&timer, send_multicast, 0);
#else
    init_timer(&timer);
    timer.function = send_multicast;
    timer.expires = 0;
    timer.data = 0;
#endif
    mod_timer(&timer, jiffies + msecs_to_jiffies(2000));
}

static int init_socket(void) {
    int error = genl_register_family(&family);
    if (error) pr_err("Family registration failed: %d\n", error);
    return error;
}

static int __init hello_init(void) {
    int error = init_socket();
    if (error) return error;
    initialize_timer();
    pr_info("Hello registered.\n");
    return 0;
}

static void __exit hello_exit(void) {
    del_timer_sync(&timer);
    genl_unregister_family(&family);
    pr_info("Hello removed.\n");
}

module_init(hello_init);
module_exit(hello_exit);

MODULE_LICENSE("GPL");
