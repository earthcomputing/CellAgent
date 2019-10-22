# sudo env NLCB=debug NLDBG=4 ./genl_sample
# sudo env NLCB=debug NLDBG=4 ./mock_exchange
# sudo env NLCB=debug NLDBG=4 ./atomic_link

CFLAGS += -g
CFLAGS += -fPIC
CFLAGS += -I/usr/include/libnl3
LDLIBS += -lnl-genl-3 -lnl-3

BIONIC_KERNEL=$(shell uname -r)
ifneq (4.18.0-25-generic,$(BIONIC_KERNEL))
EARTH_CUSTOM=3.3.4
else
EARTH_CUSTOM=3.4.2.1
CFLAGS += -DBIONIC
endif

ENTL_E1000E=../../bjackson-e1000e/e1000e-$(EARTH_CUSTOM)/src

CFLAGS += -I$(ENTL_E1000E)
CFLAGS += -I../src

OBJS=ecnl_proto.o genl_sample.o

all: genl_sample mock_exchange route-repair

genl_sample: $(OBJS)

mock_exchange: ecnl_proto.o mock_exchange.o

atomic_link: ecnl_proto.o atomic_link.o

