# nosuid for su/sudo:
If su/sudo cannot work, especially in termux, please try remounting the filesystem without suid:     
```sh
mount -o remount,nosuid /data
```
# Network issues:
## First, check dns setting:
```sh
cat /etc/resolv.conf
```
If there is no valid nameserver, please add one, for example:      
```sh
rm -f /etc/resolv.conf
echo "nameserver 1.1.1.1" >> /etc/resolv.conf
```
## If still having network issues:
Try running [fixup.sh](../blob/fixup.sh) in container.
Or see [#13](https://github.com/RuriOSS/rurima/issues/13#issuecomment-3411117953)
# Network isolation:
Rurima does not provide network isolation support, a possible implementation is at [net_wrapper.sh](../blob/net_wrapper.sh)