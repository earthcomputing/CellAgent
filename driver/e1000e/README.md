
## e1000e device driver with "Atomic Link" (tm?) modifications

    In order to minimize maintenance cost, rather than putting all source files under git control, a slightly more compilicated build is used.  The build starts with media files (tarballs) downloaded directly from Intel, to which Earth Computing specific patches are applied (i.e. via a single patch file).

    The build then combines the patch intel files with a set of Earth Computing additional files.

## build steps
$ cd driver/e1000e
driver/e1000e$ make

## Copyright

    It's unclear what legal (copyright & patent) implications there are here. Before Earth Computing distributes the combined, derivitive e1000e device driver a formal legal determination needs to be made.

## media files

    5c6d010341868f753cf983cbe4467db5 media/intel-e1000e-3.3.4.tar.gz
    f3dd43249a72553a8951225d121d6227 media/intel-e1000e-3.4.0.2.tar.gz
    7503911b5cbe0b654406b791dd768874 media/intel-e1000e-3.4.2.1.tar.gz

## Authoritative Intel e1000e device driver release(s)

    https://downloadcenter.intel.com/download/15817/Intel-Network-Adapter-Driver-for-PCIe-Intel-Gigabit-Ethernet-Network-Connections-Under-Linux-?product=46827

    Version: 3.4.2.1 (Latest) Date: 8/26/2018
    MD5: 7503911b5cbe0b654406b791dd768874
    e1000e-3.4.2.1.tar.gz

    Version: 3.4.0.2 (Previously Released) Date: 10/22/2017
    MD5: f3dd43249a72553a8951225d121d6227
    e1000e-3.4.0.2.tar.gz

    Version: 3.3.4 (Previously Released) Date: 5/20/2016
    MD5: 5c6d010341868f753cf983cbe4467db5
    e1000e-3.3.4.tar.gz
