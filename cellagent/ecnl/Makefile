
all:
	gcc -c -fPIC -o ecnl_sdk.o -I/usr/include/libnl3 -I../../../NALDD/entl_drivers/ecnl_device ecnl_sdk.c
	ar -rc ecnl_sdk.a ecnl_sdk.o
#       This seems nicer, but can't reference it from rust
#	ar -rc ecnl_sdk.a ecnl_sdk.o /lib/x86_64-linux-gnu/libnl-genl-3.so /lib/x86_64-linux-gnu/libnl-3.so


