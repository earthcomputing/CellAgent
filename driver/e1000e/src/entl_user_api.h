#ifndef _ENTL_USER_API_H_
#define _ENTL_USER_API_H_

// Ethernet Protocol ID's
#define ETH_P_ECLP  0xEAC0 /* Link Protocol (Atomic) */
#define ETH_P_ECLD  0xEAC1 /* Link Discovery */
#define ETH_P_ECLL  0xEAC2 /* Link Local Delivery (virtual, Control Messages) */

// ref: entl_state_error
#define ENTL_ERROR_FLAG_SEQUENCE 0x0001
#define ENTL_ERROR_FLAG_TIMEOUT  0x0004
#define ENTL_ERROR_SAME_ADDRESS  0x0008
#define ENTL_ERROR_UNKOWN_CMD    0x0010
#define ENTL_ERROR_UNKOWN_STATE  0x0020
#define ENTL_ERROR_UNEXPECTED_LU 0x0040
#define ENTL_ERROR_FATAL         0x8000

#include "entl_ioctl.h"

#endif
