
EARTH_CUSTOM=e1000e-3.3.4
MEDIA=media/intel-$(EARTH_CUSTOM).tar.gz

ccflags-y += -std=gnu99
ccflags-y += -Wno-declaration-after-statement

CFLAGS_EXTRA="-DENTL"

default: build

# e1000e-3.3.4
expand:
	tar xf $(MEDIA)

patch: expand patch.dif
	patch -Np0 < patch.dif
	cp src/* $(EARTH_CUSTOM)/src/

build: patch
	$(MAKE) CFLAGS_EXTRA="$(CFLAGS_EXTRA) $(ccflags-y)" -C $(EARTH_CUSTOM)/src
