# unix
授予修改`/etc/hosts`和绑定443端口的权限
```bash
$ sudo zypper install libcap2
$ sudo setcap 'CAP_DAC_OVERRIDE+ep cap_net_bind_service=+ep' <executable file>
```