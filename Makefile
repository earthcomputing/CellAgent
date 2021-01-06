NR_CPU := $(shell nproc)
MK := make -j$(NR_CPU)

e1000e:
	@cd driver/e1000e && $(MK)

ecnl:
	@cd driver/ecnl/src && $(MK) && cd ../lib && $(MK)

cell:
	@cd userspace/cellagent && \
	cargo build --release --bin cell --features="cell"

load: e1000e ecnl
	@sudo insmod ./driver/e1000e/e1000e-3.3.4/src/e1000e.ko
	@sudo insmod ./driver/ecnl/src/ecnl_device.ko

netprep: 
	sudo ip link set enp6s0 up
	sudo ip link set enp7s0 up
	sudo ip link set enp8s0 up
	sudo ip link set enp9s0 up
	sudo ip link set enp6s0 promisc on
	sudo ip link set enp7s0 promisc on
	sudo ip link set enp8s0 promisc on
	sudo ip link set enp9s0 promisc on

notraffic:
	sudo pkill NetworkManager | true
	sudo systemctl disable avahi-daemon | true
	sudo pkill avahi-daemon | true
	sudo pkill avahi-daemon | true
	sudo pkill dhclient | true
	sudo ip link set enp6s0 arp off
	sudo ip link set enp7s0 arp off
	sudo ip link set enp8s0 arp off
	sudo ip link set enp9s0 arp off
	sudo ip link set enp6s0 multicast off
	sudo ip link set enp7s0 multicast off
	sudo ip link set enp8s0 multicast off
	sudo ip link set enp9s0 multicast off
	sudo ip link set enp6s0 allmulticast off
	sudo ip link set enp7s0 allmulticast off
	sudo ip link set enp8s0 allmulticast off
	sudo ip link set enp9s0 allmulticast off

cellagent: netprep notraffic
	@cd userspace/cellagent && sudo target/release/cell
