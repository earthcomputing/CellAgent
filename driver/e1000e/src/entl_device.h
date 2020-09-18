#ifndef _ENTL_DEVICE_H_
#define _ENTL_DEVICE_H_

#define ENTL_DEVICE_FLAG_HELLO   0x0001
#define ENTL_DEVICE_FLAG_SIGNAL  0x0002
#define ENTL_DEVICE_FLAG_RETRY   0x0004
#define ENTL_DEVICE_FLAG_WAITING 0x0008
#define ENTL_DEVICE_FLAG_SIGNAL2 0x0010
#define ENTL_DEVICE_FLAG_FATAL   0x8000

#include "entl_skb_queue.h"
#include "entl_state_machine.h"

typedef struct entl_mgr {
    void (*emf_event)(struct entl_mgr *self, int sigusr); // called from watchdog, be careful
    void *emf_private;
} entl_mgr_t;

typedef struct entl_device {
    int edev_action;
    uint32_t edev_flag; // ENTL_DEVICE_FLAG
    uint32_t edev_l_addr;
    char edev_name[ENTL_DEVICE_NAME_LEN]; // 15
    int edev_queue_stopped;
    entl_state_machine_t edev_stm;
    ENTL_skb_queue_t edev_tx_skb_queue;
    uint16_t edev_u_addr;
    int edev_user_pid;
    struct timer_list edev_watchdog_timer;
    struct work_struct edev_watchdog_task;
    entl_mgr_t *edev_mgr;
} entl_device_t;

#include "entl_user_api.h"
#include "netdev_entl_if.h"

#endif
