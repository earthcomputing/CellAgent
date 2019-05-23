
## Earth Computing Netlink (generic family plug-in module)

    egrep 'ECNL|ENTL|e1000' /var/log/syslog ; tail -f /var/log/syslog | egrep 'ECNL|ENTL|e1000'

    sudo insmod /home/demouser/earthcomputing/bjackson-e1000e/e1000e-3.3.4/src/e1000e.ko
    sudo insmod /home/demouser/earthcomputing/bjackson-ecnl/src/ecnl_device.ko

    sudo env NLCB=debug NLDBG=4 /home/demouser/earthcomputing/bjackson-ecnl/lib/genl_sample
    sudo env NLCB=debug /home/demouser/earthcomputing/bjackson-ecnl/lib/genl_sample
    sudo /home/demouser/earthcomputing/bjackson-ecnl/lib/genl_sample

## build info

    https://www.kernel.org/doc/Documentation/kbuild/modules.txt

