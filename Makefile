
EARTH_CUSTOM=e1000e-3.3.4
MEDIA=media/intel-$(EARTH_CUSTOM).tar.gz

default: build

# e1000e-3.3.4
expand:
	tar xf $(MEDIA)

patch: expand patch.dif
	patch -Np0 < patch.dif
	cp src/* $(EARTH_CUSTOM)/src/

build: patch
	$(MAKE) -C $(EARTH_CUSTOM)/src
