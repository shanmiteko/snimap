# Reverse Proxy
[![ci](https://github.com/shanmiteko/snimap/actions/workflows/ci.yml/badge.svg)](https://github.com/shanmiteko/snimap/actions/workflows/ci.yml)

```bash
$ sudo zypper install libcap2
$ cargo build
$ sudo setcap 'CAP_DAC_OVERRIDE+ep cap_net_bind_service=+ep' <executable file>
$ cargo run
```