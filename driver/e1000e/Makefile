#---------------------------------------------------------------------------------------------
 #  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 #  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
#---------------------------------------------------------------------------------------------

# 4.4.0-47-generic - alice/bob
# 4.4.0-134-generic - carol
# 4.18.0-25-generic - bionic (release)
# 5.0.0-29-generic - bionic (update)

BIONIC_KERNEL=$(shell uname -r)
ifeq (5.0.0-29-generic,$(BIONIC_KERNEL))
EARTH_CUSTOM=3.4.2.4
else
ifeq (4.18.0-25-generic,$(BIONIC_KERNEL))
EARTH_CUSTOM=3.4.2.1
else
# 4.4.0-134-generic
# 4.4.0-47-generic
EARTH_CUSTOM=3.3.4
endif
endif

MEDIA=media/intel-e1000e-$(EARTH_CUSTOM).tar.gz

ccflags-y += -std=gnu99
ccflags-y += -Wno-declaration-after-statement
ccflags-y += -Wno-unused-variable

CFLAGS_EXTRA="-DENTL"

default: build

# e1000e-3.3.4
expand:
	tar xf $(MEDIA)

patch: expand patch-$(EARTH_CUSTOM).dif
	patch -Np0 < patch-$(EARTH_CUSTOM).dif
	cp src/* e1000e-$(EARTH_CUSTOM)/src/

build: patch
	$(MAKE) CFLAGS_EXTRA="$(CFLAGS_EXTRA) $(ccflags-y)" -C e1000e-$(EARTH_CUSTOM)/src
