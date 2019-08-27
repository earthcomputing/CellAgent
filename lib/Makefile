# sudo env NLCB=debug NLDBG=4 ./genl_sample
# sudo env NLCB=debug NLDBG=4 ./mock_exchange

CFLAGS += -I/usr/include/libnl3
LDLIBS += -lnl-genl-3 -lnl-3

CFLAGS += -I/home/demouser/earthcomputing/bjackson-e1000e/e1000e-3.3.4/src
CFLAGS += -I../src

OBJS=ecnl_proto.o genl_sample.o

all: genl_sample mock_exchange

genl_sample: $(OBJS)

mock_exchange: ecnl_proto.o mock_exchange.o

