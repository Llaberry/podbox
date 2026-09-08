cd ${TEST_ROOT}
source global.sh

export TEST_NO=10
export DESCRIPTION="Test if pid file works properly"
show_test_description

export SUBTEST_NO=1
export SUBTEST_DESCRIPTION="RURI_EXITED_0"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_EXITED_0 ./test true
check_if_succeed $?
./ruri -U ./test
pass_subtest

export SUBTEST_NO=2
export SUBTEST_DESCRIPTION="RURI_EXITED_1"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_EXITED_1 ./test false
check_if_succeed $?
./ruri -U ./test
pass_subtest

export SUBTEST_NO=3
export SUBTEST_DESCRIPTION="RURI_PANIC_EXE"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_PANIC_EXE ./test kytfgg
check_if_succeed $?
./ruri -U ./test
pass_subtest

export SUBTEST_NO=4
export SUBTEST_DESCRIPTION="RURI_PANIC_TIMEOUT"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_PANIC_TIMEOUT --timeout 1 ./test sleep 10
check_if_succeed $?
./ruri -U ./test
pass_subtest

export SUBTEST_NO=5
export SUBTEST_DESCRIPTION="RURI_EXITED_114"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_EXITED_114 ./test sh -c "exit 114"
check_if_succeed $?
./ruri -U ./test
pass_subtest

export SUBTEST_NO=6
export SUBTEST_DESCRIPTION="RURI_EXITED_0 with unshare"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_EXITED_0 -u ./test true
check_if_succeed $?
./ruri -U ./test
pass_subtest

export SUBTEST_NO=7
export SUBTEST_DESCRIPTION="RURI_EXITED_1 with unshare"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_EXITED_1 -u ./test false
check_if_succeed $?
./ruri -U ./test
pass_subtest

export SUBTEST_NO=8
export SUBTEST_DESCRIPTION="RURI_PANIC_EXE with unshare"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_PANIC_EXE -u ./test kytfgg
check_if_succeed $?
./ruri -U ./test
pass_subtest

export SUBTEST_NO=9
export SUBTEST_DESCRIPTION="RURI_PANIC_TIMEOUT with unshare"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_PANIC_TIMEOUT -u --timeout 1 ./test sleep 10
check_if_succeed $?
./ruri -U ./test
pass_subtest

export SUBTEST_NO=10
export SUBTEST_DESCRIPTION="RURI_EXITED_114 with unshare"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_EXITED_114 -u ./test sh -c "exit 114"
check_if_succeed $?
./ruri -U ./test
pass_subtest

export SUBTEST_NO=11
export SUBTEST_DESCRIPTION="--auto-umount"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_EXITED_0 --auto-umount ./test true
check_if_succeed $?
sleep 3
if [[ -e ./test/.rurienv ]]; then
    error "Container config file should be removed after auto-umount!"
fi
./ruri -U ./test
pass_subtest

export SUBTEST_NO=12
export SUBTEST_DESCRIPTION="--umount-on-panic"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_PANIC_EXE --umount-on-panic ./test jhbvvjhvb
check_if_succeed $?
sleep 3
if [[ -e ./test/.rurienv ]]; then
    error "Container config file should be removed after umount-on-panic!"
fi
./ruri -U ./test
pass_subtest

export SUBTEST_NO=13
export SUBTEST_DESCRIPTION="--umount-on-panic with --timeout"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_PANIC_TIMEOUT --umount-on-panic --timeout 1 ./test sleep 10
check_if_succeed $?
sleep 3
if [[ -e ./test/.rurienv ]]; then
    error "Container config file should be removed after umount-on-panic!"
fi
./ruri -U ./test
pass_subtest

export SUBTEST_NO=14
export SUBTEST_DESCRIPTION="--auto-umount with -u"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_EXITED_0 -u --auto-umount ./test true
check_if_succeed $?
sleep 3
if [[ -e ./test/.rurienv ]]; then
    error "Container config file should be removed after auto-umount!"
fi
./ruri -U ./test
pass_subtest

export SUBTEST_NO=15
export SUBTEST_DESCRIPTION="--umount-on-panic with -u"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_PANIC_EXE -u --umount-on-panic ./test jhbvvjhvb
check_if_succeed $?
sleep 3
if [[ -e ./test/.rurienv ]]; then
    error "Container config file should be removed after umount-on-panic!"
fi
./ruri -U ./test
pass_subtest

export SUBTEST_NO=16
export SUBTEST_DESCRIPTION="--umount-on-panic with --timeout with -u"
show_subtest_description
cd ${TMPDIR}
./test_pid_file RURI_PANIC_TIMEOUT -u --umount-on-panic --timeout 1 ./test sleep 10
check_if_succeed $?
sleep 3
if [[ -e ./test/.rurienv ]]; then
    error "Container config file should be removed after umount-on-panic!"
fi
./ruri -U ./test
pass_subtest

pass_test
