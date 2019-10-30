CONFIG_MODULE_SIG=n
KDIR := /lib/modules/$(shell uname -r)/build
PWD       := $(shell pwd)

ccflags-y += -std=gnu99
ccflags-y += -Wno-declaration-after-statement
ccflags-y += -Wno-unused-variable
ccflags-y += -Wno-unused-function
# -Wframe-larger-than=

## EXTRA_CFLAGS += -I$(src)
## generated-y += -I$(src)

ifeq (4.18.0-25-generic,$(BIONIC_KERNEL))
CFLAGS += -DBIONIC
endif


obj-m += kern.o

all:
	echo $(EXTRA_CFLAGS)
	$(MAKE) -C $(KDIR) M=$(PWD) modules
	$(MAKE) -f Makefile.user
