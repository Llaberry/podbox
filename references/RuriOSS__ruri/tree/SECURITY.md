# For kernel 0-days:
We are working for default seccomp profile, but unfortunately, the profile is not fully tested, and we cannot enforce it by default now. You can use `--enable-seccomp` to enable it, and help us to test it.       
# Security Policy
As ruri is not a project with a large user base, there is no formal security policy in place. Just report the issues to us, and we will try to fix them as soon as possible.

## Supported Versions
Only the latest version is supported now. As this project is backward compatible, no backports will be applied.      

## Reporting a Vulnerability

As ruri is an experimental project, you can just report them whether in issues, pull requests or email moe-hacker@outlook.com    

# Fix and Disclosure Process:
Emmmmm, when @Moe-hacker have time, he will fix the issues and release a new version. There is no formal timeline for this process.       
Give some time for fixing and testing. After the fix is released, the issues will be disclosed in this file.      

# Security Issues:
### <=3.9.0:
- CAP_SYS_CHROOT is kept by default, which may lead to container escape.
- - Seriousness: Very High
- - Fix: >=3.9.1: CAP_SYS_CHROOT is dropped by default now.
### <=3.9.1: 
- ruri did not reload itself from memfd, which may lead to information leak or even code injection if /proc/pid/exe is replaced.
- - Seriousness: Low
- - Fix: >=3.9.2: ruri will reload itself from memfd now.
### <=3.9.2: 
- For musl based systems, if a user belongs to more than 32 groups, it will panic due to array overflow when parsing /etc/group.
- - Seriousness: Medium
- - Fix: >=3.9.3: This issue is fixed in v3.9.3 and later.
- - Special Thanks: Thanks to @docensia, his report helps a lot.