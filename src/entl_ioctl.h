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
    uint32_t num_queued;
    uint32_t message_len;
    char data[MAX_AIT_MESSAGE_SIZE];
} entt_ioctl_ait_data_t;

#endif
