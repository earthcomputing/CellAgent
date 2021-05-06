/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
#ifndef _ENTL_IOCTL_H_
#define _ENTL_IOCTL_H_

// IOCTL cmd values
// ref: netdev.c
// #define SIOCDEVPRIVATE_ENTL 0x89F0 /* to 89FF */
#define SIOCDEVPRIVATE_ENTL_RD_CURRENT  0x89F0
#define SIOCDEVPRIVATE_ENTL_RD_ERROR    0x89F1
#define SIOCDEVPRIVATE_ENTL_SET_SIGRCVR 0x89F2
#define SIOCDEVPRIVATE_ENTL_GEN_SIGNAL  0x89F3
#define SIOCDEVPRIVATE_ENTL_DO_INIT     0x89F4
#define SIOCDEVPRIVATE_ENTT_SEND_AIT    0x89F5
#define SIOCDEVPRIVATE_ENTT_READ_AIT    0x89F6

// IOCTL signal event
// ref: ENTL_ACTION_PROC_AIT, entl_device_process_rx_packet, entl_new_AIT_message
#define MAX_AIT_MESSAGE_SIZE 256

// FIXME: unused num_messages, num_queued?
typedef struct entt_ioctl_ait_data {
    uint32_t num_messages;
    uint32_t message_len;
    char data[MAX_AIT_MESSAGE_SIZE];
    uint32_t num_queued;
} entt_ioctl_ait_data_t;

#include "entl_state.h"

// FIXME: entl_state_t
typedef struct entl_ioctl_data {
    int pid;
    int link_state; // 0: down, 1: up
    entl_state_t state;
    entl_state_t error_state;
    uint32_t icr;
    uint32_t ctrl;
    uint32_t ims;
    uint32_t num_queued;
} entl_ioctl_data_t;

#endif
