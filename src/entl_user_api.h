#ifndef _ENTL_USER_API_H_
#define _ENTL_USER_API_H_

// Ethernet Protocol ID's
#define ETH_P_ECLP  0xEAC0 /* Link Protocol (Atomic) */
#define ETH_P_ECLD  0xEAC1 /* Link Discovery */
#define ETH_P_ECLL  0xEAC2 /* Link Local Delivery (virtual, Control Messages) */

// IOCTL cmd values
// #define SIOCDEVPRIVATE_ENTL 0x89F0 /* to 89FF */
#define SIOCDEVPRIVATE_ENTL_RD_CURRENT  0x89F0
#define SIOCDEVPRIVATE_ENTL_RD_ERROR    0x89F1
#define SIOCDEVPRIVATE_ENTL_SET_SIGRCVR 0x89F2
#define SIOCDEVPRIVATE_ENTL_GEN_SIGNAL  0x89F3
#define SIOCDEVPRIVATE_ENTL_DO_INIT     0x89F4
#define SIOCDEVPRIVATE_ENTT_SEND_AIT    0x89F5
#define SIOCDEVPRIVATE_ENTT_READ_AIT    0x89F6

#endif
