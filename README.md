# Snimap - Reverse Proxy
[![ci](https://github.com/shanmiteko/snimap/actions/workflows/ci.yml/badge.svg)](https://github.com/shanmiteko/snimap/actions/workflows/ci.yml)
[![build](https://github.com/shanmiteko/snimap/actions/workflows/release.yml/badge.svg)](https://github.com/shanmiteko/snimap/actions/workflows/release.yml)

白名单机制的本地HTTPS反向代理，可对HTTPS请求的[SNI](https://en.wikipedia.org/wiki/Server_Name_Indication)进行[修改](https://en.wikipedia.org/wiki/Domain_fronting)或去除以突破互联网审查

修复因DNS污染，SNI阻断导致的访问问题

已[默认](./src/config/format.rs#L187)添加
- [duckduckgo](https://duckduckgo.com)
- [github](https://github.com)
- [onedrive](https://onedrive.com)
- [wikipedia](https://wikipedia.com)
- [pixiv](https://pixiv.net)
- [twitch](https://twitch.tv)

可在[配置文件](#配置文件)中添加更多网站

## How to use
**安装ssl根证书**

证书文件`ca.crt`

**运行snimap**

授予执行权限

### windows
以管理员方式运行

### linux
使用`sudo`

*注意此时的配置文件在root用户目录内*

或者使用`setcap`授予更小粒度的权限 ([capabilities](https://man7.org/linux/man-pages/man7/capabilities.7.html))
```
$ sudo setcap 'cap_dac_override+ep cap_net_bind_service=+ep' ./target/release/snimap
```
完成后即可用非root用户运行

## 配置文件

**配置文件位置**

`${config_dir}/snimap/config.toml`

| Platform | Value                                 | Example                                  |
| -------- | ------------------------------------- | ---------------------------------------- |
| Linux    | `$XDG_CONFIG_HOME` or `$HOME`/.config | /home/alice/.config                      |
| macOS    | `$HOME`/Library/Application Support   | /Users/Alice/Library/Application Support |
| Windows  | `{FOLDERID_RoamingAppData}`           | C:\Users\Alice\AppData\Roaming           |

**配置文件格式**

相关代码

`enable`和`enable_sni`默认为`true`

```rs
#[derive(Deserialize, Serialize)]
pub struct Config {
    enable: Option<bool>,
    enable_sni: Option<bool>,
    groups: Vec<Group>,
}

#[derive(Deserialize, Serialize)]
pub struct Group {
    enable: Option<bool>,
    enable_sni: Option<bool>,
    name: String,
    sni: Option<String>,
    mappings: Vec<Mapping>,
}

#[derive(Deserialize, Serialize)]
pub struct Mapping {
    enable: Option<bool>,
    enable_sni: Option<bool>,
    hostname: String,
    sni: Option<String>,
}
```

Example

```toml
[[groups]]
name = "Duckduckgo"

[[groups.mappings]]
hostname = "duckduckgo.com"

[[groups]]
enable_sni = false
name = "Wikipedia"

[[groups.mappings]]
hostname = "zh.wikipedia.org"

[[groups]]
name = "Pixiv"

[[groups.mappings]]
hostname = "pixiv.net"
sni = "www.fanbox.cc"
```

## How to Build
On OpenSUSE
```bash
$ git clone git@github.com:shanmiteko/snimap.git --depth=1
$ sudo zypper install libopenssl-devel
$ cargo build --release
$ sudo zypper install ibcap2 libcap-progs
$ sudo setcap 'cap_dac_override+ep cap_net_bind_service=+ep' ./target/release/snimap
$ cargo run --release
```
