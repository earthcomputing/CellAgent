#!/bin/csh -f

#set alice = 172.16.1.67
set bob = 172.16.1.40
set carol = 172.16.1.105

set left = ${bob}
set right = ${carol}

if ( $#argv > 0 ) then
    set left = $1:q
    set right = $2:q
endif

# set WS = /home/bjackson/earth-computing/git-projects
# set KVER = 3.4.2.1

set WS = /home/demouser/earthcomputing
set KVER = 3.3.4

set cmd_dir = /tmp/repair
mkdir -p ${cmd_dir}

set squot = "'"

# set position and logical size
echo "printf '\e[3;0;0t' ; printf '\e[8;15;95t'" > ${cmd_dir}/bob-syslog.command
echo "printf '\e[3;0;250t' ; printf '\e[8;15;95t'" > ${cmd_dir}/bob-phase1.command
echo "printf '\e[3;0;500t' ; printf '\e[8;15;95t'" > ${cmd_dir}/bob-phase2.command

echo "printf '\e[3;700;0t' ; printf '\e[8;15;95t'" > ${cmd_dir}/carol-syslog.command
echo "printf '\e[3;700;250t' ; printf '\e[8;15;95t'" > ${cmd_dir}/carol-phase1.command
echo "printf '\e[3;700;500t' ; printf '\e[8;15;95t'" > ${cmd_dir}/carol-phase2.command

echo 'ssh -t demouser@'${bob}' "tail -f  /var/log/syslog | egrep -v '${squot}'NetworkManager|avahi-daemon'${squot}'"' >> ${cmd_dir}/bob-syslog.command
echo 'ssh -t demouser@'${bob}' "sudo insmod '${WS}'/bjackson-e1000e/e1000e-'${KVER}'/src/e1000e.ko"' >> ${cmd_dir}/bob-phase1.command
echo 'ssh -t demouser@'${bob}' "sudo '${WS}'/bjackson-ecnl/lib/route-repair"' >> ${cmd_dir}/bob-phase1.command
echo 'ssh -t demouser@'${bob}' "sudo insmod '${WS}'/bjackson-ecnl/src/ecnl_device.ko"' >> ${cmd_dir}/bob-phase2.command
echo 'ssh -t demouser@'${bob}' "sudo '${WS}'/bjackson-ecnl/lib/mock_exchange"' >> ${cmd_dir}/bob-phase2.command

echo 'ssh -t demouser@'${carol}' "tail -f  /var/log/syslog | egrep -v '${squot}'NetworkManager|avahi-daemon'${squot}'"' >> ${cmd_dir}/carol-syslog.command
echo 'ssh -t demouser@'${carol}' "sudo insmod '${WS}'/bjackson-e1000e/e1000e-'${KVER}'/src/e1000e.ko"' >> ${cmd_dir}/carol-phase1.command
echo 'ssh -t demouser@'${carol}' "sudo '${WS}'/bjackson-ecnl/lib/route-repair"' >> ${cmd_dir}/carol-phase1.command
echo 'ssh -t demouser@'${carol}' "sudo insmod '${WS}'/bjackson-ecnl/src/ecnl_device.ko"' >> ${cmd_dir}/carol-phase2.command
echo 'ssh -t demouser@'${carol}' "sudo '${WS}'/bjackson-ecnl/lib/mock_exchange"' >> ${cmd_dir}/carol-phase2.command

chmod +x ${cmd_dir}/*.command
open ${cmd_dir}/

exit 0

open ${cmd_dir}/bob-syslog.command
open ${cmd_dir}/bob-phase1.command
open ${cmd_dir}/bob-phase2.command

sleep 5

open ${cmd_dir}/carol-syslog.command
open ${cmd_dir}/carol-phase1.command
open ${cmd_dir}/carol-phase2.command

# eof
