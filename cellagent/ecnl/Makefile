# sudo env NLCB=debug NLDBG=4 ./genl_sample
# sudo env NLCB=debug NLDBG=4 ./mock_exchange

CFLAGS += -I/usr/include/libnl3
LDLIBS += -lnl-genl-3 -lnl-3

BIONIC_KERNEL=$(shell uname -r)
ifneq (4.18.0-25-generic,$(BIONIC_KERNEL))
EARTH_CUSTOM=3.3.4
else
EARTH_CUSTOM=3.4.2.1
endif

ENTL_E1000E=../../../bjackson-e1000e/e1000e-$(EARTH_CUSTOM)/src
ENTL_ECNL_LIB=../../../bjackson-ecnl/lib
ENTL_ECNL_SRC=../../../bjackson-ecnl/src

CFLAGS += -I$(ENTL_E1000E)
CFLAGS += -I$(ENTL_ECNL_SRC)
CFLAGS += -I$(ENTL_ECNL_LIB)
LDLIBS += -l$(ENTL_ECNL_LIB)

all:
	gcc -c -fPIC -o ecnl_sdk.o $(CFLAGS) ecnl_sdk.c
#       This seems nicer, but can't reference it from rust
#	ar -rc ecnl_sdk.a ecnl_sdk.o /lib/x86_64-linux-gnu/libnl-genl-3.so /lib/x86_64-linux-gnu/libnl-3.so


