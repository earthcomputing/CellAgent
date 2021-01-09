BASE := $(PWD)
NR_CPU := $(shell nproc)

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


PATCHDIR := patch.d/$(EARTH_CUSTOM)
PATCHNR = $(shell ls $(BASE)/patch.d/3.3.4 | sort -n -r | grep -o -E '[0-9]+' | head -1)

default: build 

expand:
	tar xf $(MEDIA)

diff: clean
	diff -r -u e1000e-$(EARTH_CUSTOM)/.src_copy e1000e-$(EARTH_CUSTOM)/src > \
	patch.d/$(EARTH_CUSTOM)/$$(($(PATCHNR) + 1)) | true
	patch -d e1000e-$(EARTH_CUSTOM)/.src_copy < $(BASE)/patch.d/$(EARTH_CUSTOM)/$$(($(PATCHNR) + 1))

patch: expand
	for f in `ls $(PATCHDIR) | sort`; do patch -Np0 < $(PATCHDIR)/$$f; done
	cp src/* e1000e-$(EARTH_CUSTOM)/src/
	cp -r e1000e-$(EARTH_CUSTOM)/src e1000e-$(EARTH_CUSTOM)/.src_copy

build: patch
	$(MAKE) -j $(NR_CPU) CFLAGS_EXTRA="$(CFLAGS_EXTRA) $(ccflags-y)" -C e1000e-$(EARTH_CUSTOM)/src

rebuild:
	$(MAKE) -j $(NR_CPU) CFLAGS_EXTRA="$(CFLAGS_EXTRA) $(ccflags-y)" -C e1000e-$(EARTH_CUSTOM)/src

dev-deps:
	sudo apt install linux-headers | true
	sudo apt install linux-source | true
	sudo apt install python-pip | true
	pip install compiledb | true


vs-compile-commands:
	cd e1000e-$(EARTH_CUSTOM)/src && compiledb make -C ../../ rebuild

vscode: vs-compile-commands
	cat misc/.vscode/c_cpp_properties.json | sed s/KERNELREL/$(EARTH_CUSTOM)/ > misc/.vscode/c_cpp_properties.json
	cp -r misc/.vscode e1000e-$(EARTH_CUSTOM)/src/
	code e1000e-$(EARTH_CUSTOM)/src/

clean:
	rm -f patch.d/$(EARTH_CUSTOM)/*.* | true 
	rm -f e1000e-$(EARTH_CUSTOM)/src/*.ko | true
	rm -f e1000e-$(EARTH_CUSTOM)/src/*.o | true
	rm -f e1000e-$(EARTH_CUSTOM)/src/.*.mod* | true
	rm -f e1000e-$(EARTH_CUSTOM)/src/.*.o | true
	rm -f e1000e-$(EARTH_CUSTOM)/src/.*.cmd | true
	rm -rf e1000e-$(EARTH_CUSTOM)/src/.tmp_versions | true
	rm -f e1000e-$(EARTH_CUSTOM)/src/*odule* | true 
	rm -f e1000e-$(EARTH_CUSTOM)/src/*.mod | true 
	rm -f e1000e-$(EARTH_CUSTOM)/src/*.mod.* | true 
	rm -f e1000e-$(EARTH_CUSTOM)/src/*.cmd | true 
	rm -f e1000e-$(EARTH_CUSTOM)/src/*.orig | true 
	rm -f e1000e-$(EARTH_CUSTOM)/src/*.rej | true 
	rm -f e1000e-$(EARTH_CUSTOM)/src/compile_commands.json | true 
	mv e1000e-$(EARTH_CUSTOM)/src/.vscode misc/.vscode
