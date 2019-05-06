#ifndef _ENTL_USER_API_H_
#define _ENTL_USER_API_H_

// Ethernet Protocol ID's
#define ETH_P_ECLP  0xEAC0 /* Link Protocol (Atomic) */
#define ETH_P_ECLD  0xEAC1 /* Link Discovery */
#define ETH_P_ECLL  0xEAC2 /* Link Local Delivery (virtual, Control Messages) */

// ref: entl_state_error
#define ENTL_ERROR_FLAG_SEQUENCE 0x0001
#define ENTL_ERROR_FLAG_LINKDONW 0x0002
#define ENTL_ERROR_FLAG_TIMEOUT  0x0004
#define ENTL_ERROR_SAME_ADDRESS  0x0008
#define ENTL_ERROR_UNKOWN_CMD    0x0010
#define ENTL_ERROR_UNKOWN_STATE  0x0020
#define ENTL_ERROR_UNEXPECTED_LU 0x0040
#define ENTL_ERROR_FATAL         0x8000

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
    u32 num_messages;
    u32 num_queued;
    u32 message_len;
    char data[MAX_AIT_MESSAGE_SIZE];
} entt_ioctl_ait_data_t;

#endif
