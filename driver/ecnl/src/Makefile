#---------------------------------------------------------------------------------------------
 #  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 #  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
#---------------------------------------------------------------------------------------------
CONFIG_MODULE_SIG=n
KDIR := /lib/modules/$(shell uname -r)/build
PWD       := $(shell pwd)

ccflags-y += -std=gnu99
ccflags-y += -Wno-declaration-after-statement
ccflags-y += -Wno-unused-variable
ccflags-y += -Wno-unused-function
# -Wframe-larger-than=

BIONIC_KERNEL=$(shell uname -r)
ifneq (4.18.0-25-generic,$(BIONIC_KERNEL))
EARTH_CUSTOM=3.3.4
else
EARTH_CUSTOM=3.4.2.1
ccflags-y += -DBIONIC
endif

ENTL_E1000E=../../e1000e/e1000e-$(EARTH_CUSTOM)/src
KBUILD_EXTRA_SYMBOLS += $(src)/$(ENTL_E1000E)/Module.symvers

EXTRA_CFLAGS += -I$(src)/$(ENTL_E1000E)
generated-y += -I$(src)/$(ENTL_E1000E)

obj-m += ecnl_device.o

all:
	echo $(EXTRA_CFLAGS)
	$(MAKE) -C $(KDIR) M=$(PWD) modules

clean:
	rm -rf *.o *.ko *.mod.* *.cmd .module* modules* Module* .*.cmd .tmp*
	make -C /lib/modules/$(shell uname -r)/build M=$(PWD) clean
