
# 4.4.0-47-generic 	- alice/bob
# 4.4.0-134-generic 	- carol
# 4.18.0-25-generic 	- Bionic (release)
# 5.0.0-29-generic 	- Bionic (update)
# 5.4.0			- Hedgehog (release)

BIONIC_KERNEL=$(shell uname -r)
ifeq (5.0.0-29-generic,$(BIONIC_KERNEL))
EARTH_CUSTOM=3.4.2.4
else ifeq (5.4.0,$(BIONIC_KERNEL))
EARTH_CUSTOM=3.8.4			# hedgehog
else ifeq (4.18.0-25-generic,$(BIONIC_KERNEL))
EARTH_CUSTOM=3.4.2.1
else
# 4.4.0-134-generic
# 4.4.0-47-generic
EARTH_CUSTOM=3.3.4
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
