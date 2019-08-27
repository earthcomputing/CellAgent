
## WARNING

    For the time being leave the old driver in initramfs - i.e. the Atsushi driver.
    Development is done by blacklisting the initramfs driver and dynamically loading driver(s).

    find /lib/modules/$(uname -r) -name e1000e.ko -ls

        6039511    428 -rw-r--r--   1 root     root       435086 Nov 14  2016 /lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko

    sha1sum /lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko

        de8262bf8f889d6d2b2ccf9a31cbe74306b0a31c  /lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko

    lsinitramfs -l /boot/initrd.img-$(uname -r) | grep e1000

        drwxr-xr-x   2 root     root            0 Aug 15 13:40 lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000
        -rw-r--r--   1 root     root       223374 Nov 14  2016 lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000/e1000.ko
        drwxr-xr-x   2 root     root            0 Aug 15 13:40 lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000e
        -rw-r--r--   1 root     root       435086 Nov 14  2016 lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko
        -rw-r--r--   1 root     root           18 Jul 31 17:56 etc/modprobe.d/e1000e.conf

## testing hints

    echo "blacklist e1000e" >> /etc/modprobe.d/e1000e.conf
    sudo depmod -ae
    
## when we're completely happy with things, install driver(s) into initramfs
    
    update-initramfs -u

## Earth Computing Netlink (generic family plug-in module)

    egrep 'ADAPT|ECNL|ENTL|e1000' /var/log/syslog ; tail -f /var/log/syslog | egrep 'ADAPT|ECNL|ENTL|e1000'

    sudo insmod /home/demouser/earthcomputing/bjackson-e1000e/e1000e-3.3.4/src/e1000e.ko
    sudo insmod /home/demouser/earthcomputing/bjackson-ecnl/src/ecnl_device.ko

    sudo env NLCB=debug NLDBG=4 /home/demouser/earthcomputing/bjackson-ecnl/lib/genl_sample
    sudo env NLCB=debug /home/demouser/earthcomputing/bjackson-ecnl/lib/genl_sample
    sudo /home/demouser/earthcomputing/bjackson-ecnl/lib/genl_sample

## build info

    https://www.kernel.org/doc/Documentation/kbuild/modules.txt

