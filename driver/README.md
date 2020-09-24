This is a subsytem for anything related to Earth Computing link protocols (ECLP).  Models of protocols should be checked directly into driver-master unless specific to particular code in a lower-level branch.


## WARNING

    For the time being leave the old driver in initramfs - i.e. the Atsushi driver.
    Development is done by blacklisting the initramfs driver and dynamically loading driver(s).

## to avoid loading old demo driver
    $ echo "blacklist e1000e" >> /etc/modprobe.d/e1000e.conf
## or edit (using sudo) /etc/modprobe.d/e1000e.conf to uncomment "blacklist e1000e".
## then:
    $ sudo depmod -ae && sudo update-initramfs -u
## Then reboot.  TO RETURN TO OLD-DEMO BEHAVIOR, RECOMMENT THE LINE IN /etc/modprobe.d/e1000e.conf

## on startup, dynamically load kernel modules
    $ cd driver
    driver$ sudo insmod e1000e/e1000e-3.3.4/src/e1000e.ko
    driver$ sudo insmod ecnl/src/ecnl_device.ko

## testing hints
    $ egrep 'ADAPT|ECNL|ENTL|e1000' /var/log/syslog ; tail -f /var/log/syslog | egrep 'ADAPT|ECNL|ENTL|e1000'

## Find the modules
    $ find /lib/modules/$(uname -r) -name e1000e.ko -ls

        6039511    428 -rw-r--r--   1 root     root       435086 Nov 14  2016 /lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko

    sha1sum /lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko

        de8262bf8f889d6d2b2ccf9a31cbe74306b0a31c  /lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko

    $ lsinitramfs -l /boot/initrd.img-$(uname -r) | grep e1000

        drwxr-xr-x   2 root     root            0 Aug 15 13:40 lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000
        -rw-r--r--   1 root     root       223374 Nov 14  2016 lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000/e1000.ko
        drwxr-xr-x   2 root     root            0 Aug 15 13:40 lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000e
        -rw-r--r--   1 root     root       435086 Nov 14  2016 lib/modules/4.4.0-47-generic/kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko
        -rw-r--r--   1 root     root           18 Jul 31 17:56 etc/modprobe.d/e1000e.conf

## Ubuntu Bionic

    $ find /lib/modules/$(uname -r) -name e1000e.ko -ls

        4856165    416 -rw-r--r--   1 root     root       422670 Jun 27 00:04 /lib/modules/4.18.0-25-generic/kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko

    sha1sum /lib/modules/4.18.0-25-generic/kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko

        81a3e1dc67a06be98a48bfae34f1c69c3daabe67  /lib/modules/4.18.0-25-generic/kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko

    $ lsinitramfs -l /boot/initrd.img-$(uname -r) | grep e1000

        drwxr-xr-x   2 root     root            0 Jul 12 20:08 lib/modules/4.18.0-25-generic/kernel/drivers/net/ethernet/intel/e1000
        -rw-r--r--   1 root     root       229806 Jun 27 00:04 lib/modules/4.18.0-25-generic/kernel/drivers/net/ethernet/intel/e1000/e1000.ko
        drwxr-xr-x   2 root     root            0 Jul 12 20:08 lib/modules/4.18.0-25-generic/kernel/drivers/net/ethernet/intel/e1000e
        -rw-r--r--   1 root     root       422670 Jun 27 00:04 lib/modules/4.18.0-25-generic/kernel/drivers/net/ethernet/intel/e1000e/e1000e.ko

    dmesg | grep e1000

        [    2.795281] e1000e: Intel(R) PRO/1000 Network Driver - 3.2.6-k
        [    2.795281] e1000e: Copyright(c) 1999 - 2015 Intel Corporation.
        [    2.795462] e1000e 0000:00:1f.6: Interrupt Throttling Rate (ints/sec) set to dynamic conservative mode
        [    3.000319] e1000e 0000:00:1f.6 0000:00:1f.6 (uninitialized): registered PHC clock
        [    3.068016] e1000e 0000:00:1f.6 eth0: (PCI Express:2.5GT/s:Width x1) 94:c6:91:ae:87:84
        [    3.068018] e1000e 0000:00:1f.6 eth0: Intel(R) PRO/1000 Network Connection
        [    3.068098] e1000e 0000:00:1f.6 eth0: MAC: 13, PHY: 12, PBA No: FFFFFF-0FF
        [    3.068825] e1000e 0000:00:1f.6 eno1: renamed from eth0

        [126766.369285] e1000e: eno1 NIC Link is Up 100 Mbps Full Duplex, Flow Control: None
        [126766.369290] e1000e 0000:00:1f.6 eno1: 10/100 speed: disabling TSO
        [128334.328402] e1000e: eno1 NIC Link is Down

