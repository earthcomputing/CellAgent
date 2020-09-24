## Earth Computing Netlink (generic family plug-in module)

    $ sudo env NLCB=debug NLDBG=4 driver/ecnl/lib/genl_sample
    $ sudo env NLCB=debug driver/ecnl/lib/genl_sample
    $ sudo driver/ecnl/lib/genl_sample

## build info

    https://www.kernel.org/doc/Documentation/kbuild/modules.txt

    $ cd driver/ecnl/src
    driver/ecnl/src$ make
    driver/ecnl/src$ cd ../lib
    driver/ecnl/lib$ make
