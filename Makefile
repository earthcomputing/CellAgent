all: help

help:
	@printf "\t\t\tMain Targets\n****************************************\n"
	@printf "\tmake drivers\n"
	@printf "\t\tbuild e1000e and ecnl drivers\n"
	@printf "\n\tmake load\n"
	@printf "\t\tload the drivers\n"
	@printf "\n\tmake cell\n"
	@printf "\t\tbuild cellagent\n"
	@printf "\n\tmake cellagent\n"
	@printf "\t\trun the cell agent and open a wireshark instance\n\t\t"
	@printf "for each interface as well as open an xterm window\n\t\t"
	@printf "that clears, scans, and prints any changes to the log\n\t\t"
	@printf "relevant to our modules every 2 seconds\n"
	@printf "\n\tmake DEV=<interface> local-ip-network\n"
	@printf "\t\tset up ip configuration for switched-ping demo\n"
	@echo ""
	
	
MK := bash -c "make -j$(shell nproc)" 1>/dev/null

ifeq ($(WS_PKT_CNT),)
WS_PKT_CNT := 32
endif

E1000E_PARMS := 	InterruptThrottleRate=0,0,0,0,0 \
			copybreak=0 \
			RxIntDelay=0,0,0,0,0 \
			RxAbsIntDelay=0,0,0,0,0 \
			TxIntDelay=0,0,0,0,0 \
			TxAbsIntDelay=0,0,0,0,0 \
			SmartPowerDownEnable=0,0,0,0,0 \
			Debug=16,16,16,16,16 \
			CrcStripping=0,0,0,0,0 \

# this is the minimum sizes for the send and receive socket-buffers for a socket
SK__MIN_RBUFSIZ := 4096
SK__MIN_WBUFSIZ := 8192

# number of socket buffers a socket can have in its recieve queue; we only want # one
SK__PKTS_RECVQ := 1

e1000e:
	@cd driver/e1000e && $(MK)

ecnl:
	@cd driver/ecnl/src && $(MK) && cd ../lib && rm *.o | true && $(MK)

cell:
	@cd userspace/cellagent/ecnl && $(MK) && \
	cd .. && cargo build --release --bin cell --features="cell" 

drivers: e1000e ecnl

load:
	@sudo insmod ./driver/e1000e/e1000e-3.3.4/src/e1000e.ko $(E1000E_PARMS)
	@sudo insmod ./driver/ecnl/src/ecnl_device.ko

phy-prep:
	@sudo ethtool -C enp6s0 rx-usecs 0 tx-usecs 0 2>/dev/null | true
	@sudo ethtool -C enp7s0 rx-usecs 0 tx-usecs 0 2>/dev/null | true
	@sudo ethtool -C enp8s0 rx-usecs 0 tx-usecs 0 2>/dev/null | true
	@sudo ethtool -C enp9s0 rx-usecs 0 tx-usecs 0 2>/dev/null | true

linux-netstack-prep:
	@sudo ip link set enp6s0 txqueuelen 0
	@sudo ip link set enp7s0 txqueuelen 0
	@sudo ip link set enp8s0 txqueuelen 0
	@sudo ip link set enp9s0 txqueuelen 0
	@sudo tc qdisc replace dev enp6s0 root noqueue
	@sudo tc qdisc replace dev enp7s0 root noqueue
	@sudo tc qdisc replace dev enp8s0 root noqueue
	@sudo tc qdisc replace dev enp9s0 root noqueue
	@sudo sysctl -w net.core.rmem_default=$(SK__MIN_RBUFSIZ) 1>/dev/null
	@sudo sysctl -w net.core.wmem_default=$(SK__MIN_WBUFSIZ) 1>/dev/null
	@sudo sysctl -w net.core.netdev_max_backlog=$(SK__PKTS_RECVQ) 1>/dev/null

	

link-prep:
	@sudo ip link set enp6s0 up
	@sudo ip link set enp7s0 up
	@sudo ip link set enp8s0 up
	@sudo ip link set enp9s0 up
	@sudo ip link set enp6s0 promisc on
	@sudo ip link set enp7s0 promisc on
	@sudo ip link set enp8s0 promisc on
	@sudo ip link set enp9s0 promisc on

wireshark:
	@sudo bash -c "\
		wireshark -k -c$(WS_PKT_CNT) -i enp6s0 & \
		wireshark -k -c$(WS_PKT_CNT) -i enp7s0 & \
		wireshark -k -c$(WS_PKT_CNT) -i enp8s0 & \
		wireshark -k -c$(WS_PKT_CNT) -i enp9s0 & "

scan-log:
	@sudo dmesg -C
	@xterm -e "watch -d \"dmesg | egrep 'ADAPT|e1000e|ECNL|ENTL'\"" &

prep: link-prep linux-netstack-prep phy-prep

notraffic:
	@sudo pkill NetworkManager | true
	@sudo systemctl disable avahi-daemon 1>/dev/null | true
	@sudo pkill avahi-daemon | true
	@sudo pkill avahi-daemon | true
	@sudo pkill dhclient | true
	@sudo ip link set enp6s0 arp off
	@sudo ip link set enp7s0 arp off
	@sudo ip link set enp8s0 arp off
	@sudo ip link set enp9s0 arp off
	@sudo ip link set enp6s0 multicast off
	@sudo ip link set enp7s0 multicast off
	@sudo ip link set enp8s0 multicast off
	@sudo ip link set enp9s0 multicast off
	@sudo ip link set enp6s0 allmulticast off
	@sudo ip link set enp7s0 allmulticast off
	@sudo ip link set enp8s0 allmulticast off
	@sudo ip link set enp9s0 allmulticast off

cellagent: notraffic prep wireshark scan-log
	@echo "Unplug All Ethernet Cables"
	@echo ""
	@echo "[press any key to continue]"
	@read blank
	@cd userspace/cellagent && sudo target/release/cell

config-boot:
	@sudo bash -c "echo \"blacklist e1000e\" > /etc/modprobe.d/e1000e.conf"
	@sudo bash -c "depmod -ae && update-initramfs -u"

local-ip-network:
	@sudo ./scripts/ip-net-setup $$DEV

ping-carol:
	@ping 10.0.0.1
ping-alice:
	@ping 10.0.1.1

reset: e1000e ecnl cell config-boot
	reboot
