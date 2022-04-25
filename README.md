```bash
$ sudo zypper install libcap2
$ sudo setcap 'CAP_DAC_OVERRIDE+ep cap_net_bind_service=+ep' <executable file>
```