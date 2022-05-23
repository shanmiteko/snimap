use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
};

use serde_derive::{Deserialize, Serialize};

type Hostname = String;

pub struct ConfigMapVal {
    pub address: SocketAddr,
    pub sni: Option<Hostname>,
}

pub type ConfigMap = HashMap<Hostname, ConfigMapVal>;

#[derive(Deserialize, Serialize)]
pub struct Config {
    enable: Option<bool>,
    enable_sni: Option<bool>,
    group: Vec<Group>,
}

#[derive(Deserialize, Serialize)]
pub struct Group {
    enable: Option<bool>,
    enable_sni: Option<bool>,
    name: String,
    sni_override: Option<String>,
    dns: Vec<Dns>,
}

#[derive(Deserialize, Serialize)]
pub struct Dns {
    enable: Option<bool>,
    enable_sni: Option<bool>,
    hostname: String,
    sni_override: Option<String>,
    address: Option<String>,
}

impl ConfigMapVal {
    fn new(address: String, sni: Option<String>) -> Self {
        Self {
            address: SocketAddr::new(
                address.parse::<IpAddr>().expect("cannot parse to IpAddr"),
                443,
            ),
            sni,
        }
    }
}

trait Switchable {
    fn enable(&self) -> bool;
    fn enable_sni(&self) -> bool;
}

impl Switchable for Config {
    fn enable(&self) -> bool {
        self.enable.unwrap_or(true)
    }

    fn enable_sni(&self) -> bool {
        self.enable_sni.unwrap_or(false)
    }
}

impl Switchable for Group {
    fn enable(&self) -> bool {
        self.enable.unwrap_or(true)
    }

    fn enable_sni(&self) -> bool {
        self.enable_sni.unwrap_or(false)
    }
}

impl Switchable for Dns {
    fn enable(&self) -> bool {
        self.enable.unwrap_or(true)
    }

    fn enable_sni(&self) -> bool {
        self.enable_sni.unwrap_or(false)
    }
}

impl Config {
    pub fn group_mut(&mut self) -> Option<&mut Vec<Group>> {
        if self.enable() {
            Some(self.group.as_mut())
        } else {
            None
        }
    }
}

impl Group {
    pub fn new(name: &str, dns: Vec<Dns>) -> Self {
        Self {
            name: name.to_string(),
            dns,
            enable: None,
            enable_sni: None,
            sni_override: None,
        }
    }

    pub fn dns_mut(&mut self) -> Option<&mut Vec<Dns>> {
        if self.enable() {
            Some(self.dns.as_mut())
        } else {
            None
        }
    }
}

impl Dns {
    pub fn new(hostname: &str) -> Self {
        Self {
            enable: None,
            enable_sni: None,
            hostname: hostname.to_string(),
            sni_override: None,
            address: None,
        }
    }

    pub fn hostname_ref(&self) -> Option<&str> {
        if self.enable() {
            Some(self.hostname.as_str())
        } else {
            None
        }
    }

    pub fn address_ref(&self) -> Option<&str> {
        self.address.as_deref()
    }

    pub fn set_address(&mut self, address: String) {
        self.address = Some(address);
    }
}

trait Merge {
    fn merge(&mut self, other: Self);
}

impl Merge for ConfigMap {
    fn merge(&mut self, other: Self) {
        other.into_iter().for_each(|(k, v)| {
            self.insert(k, v);
        })
    }
}

impl From<Dns> for ConfigMap {
    fn from(dns: Dns) -> Self {
        let mut config_map: ConfigMap = HashMap::new();
        if dns.enable() {
            let enable_sni = dns.enable_sni();
            let Dns {
                hostname,
                sni_override,
                address,
                ..
            } = dns;
            if let Some(addr) = address {
                config_map.insert(
                    hostname.clone(),
                    ConfigMapVal::new(addr, enable_sni.then_some(sni_override.unwrap_or(hostname))),
                );
            }
        }
        config_map
    }
}

impl From<Group> for ConfigMap {
    fn from(group: Group) -> Self {
        let mut config_map: ConfigMap = HashMap::new();
        if group.enable() {
            let enable_sni = group.enable_sni();
            let Group {
                dns, sni_override, ..
            } = group;
            dns.into_iter().for_each(|mut d: Dns| {
                d.sni_override = enable_sni.then_some(sni_override.clone()).flatten();
                config_map.merge(d.into());
            });
        }
        config_map
    }
}

impl From<Config> for ConfigMap {
    fn from(config: Config) -> Self {
        let mut config_map: ConfigMap = HashMap::new();
        if config.enable() {
            let enable_sni = config.enable_sni();
            config.group.into_iter().for_each(|mut g: Group| {
                if !enable_sni {
                    g.sni_override = None
                }
                config_map.merge(g.into());
            });
        }
        config_map
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable: None,
            enable_sni: None,
            group: vec![
                Group::new(
                    "Duckduckgo",
                    [
                        "duck.com",
                        "duckduckgo.com",
                        "external-content.duckduckgo.com",
                        "links.duckduckgo.com",
                    ]
                    .into_iter()
                    .map(Dns::new)
                    .collect(),
                ),
                Group::new(
                    "Github",
                    [
                        "github.com",
                        "avatars.githubusercontent.com",
                        "avatars0.githubusercontent.com",
                        "avatars1.githubusercontent.com",
                        "avatars2.githubusercontent.com",
                        "avatars3.githubusercontent.com",
                        "camo.githubusercontent.com",
                        "cloud.githubusercontent.com",
                        "github.githubassets.com",
                        "raw.githubusercontent.com",
                        "user-images.githubusercontent.com",
                    ]
                    .into_iter()
                    .map(Dns::new)
                    .collect(),
                ),
                Group::new(
                    "Onedrive",
                    [
                        "onedrive.com",
                        "api.onedrive.com",
                        "onedrive.live.com",
                        "skyapi.onedrive.live.com",
                    ]
                    .into_iter()
                    .map(Dns::new)
                    .collect(),
                ),
                Group::new(
                    "Wikipedia",
                    [
                        "zh.wikipedia.org",
                        "en.wikipedia.org",
                        "wikimedia.org",
                        "login.wikimedia.org",
                        "upload.wikimedia.org",
                        "maps.wikimedia.org",
                    ]
                    .into_iter()
                    .map(Dns::new)
                    .collect(),
                ),
                Group::new(
                    "Twitch",
                    [
                        "twitch.tv",
                        "www.twitch.tv",
                        "static.twitchcdn.net",
                        "gql.twitch.tv",
                    ]
                    .into_iter()
                    .map(Dns::new)
                    .collect(),
                ),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigMap, Dns, Group};

    #[test]
    fn config_default_is_ok() {
        toml::to_string_pretty(&Config::default()).unwrap();
    }

    #[test]
    fn dns_into_config_map_is_ok() {
        let config_map: ConfigMap = Dns {
            enable: Some(false),
            enable_sni: Some(false),
            hostname: "hostname".to_string(),
            sni_override: Some("sni".to_string()),
            address: Some("1.1.1.1".to_string()),
        }
        .into();
        assert_eq!(config_map.len(), 0);
        let config_map: ConfigMap = Dns {
            enable: Some(true),
            enable_sni: Some(false),
            hostname: "hostname".to_string(),
            sni_override: Some("sni".to_string()),
            address: Some("1.1.1.1".to_string()),
        }
        .into();
        assert_eq!(
            config_map.get("hostname").unwrap().address,
            "1.1.1.1:443".parse().unwrap()
        );
        assert_eq!(config_map.get("hostname").unwrap().sni, None);
        let config_map: ConfigMap = Dns {
            enable: Some(true),
            enable_sni: Some(true),
            hostname: "hostname".to_string(),
            sni_override: Some("sni".to_string()),
            address: Some("1.1.1.1".to_string()),
        }
        .into();
        assert_eq!(
            config_map.get("hostname").unwrap().sni,
            Some("sni".to_string())
        );
        let config_map: ConfigMap = Dns {
            enable: Some(true),
            enable_sni: Some(true),
            hostname: "hostname".to_string(),
            sni_override: None,
            address: Some("1.1.1.1".to_string()),
        }
        .into();
        assert_eq!(
            config_map.get("hostname").unwrap().sni,
            Some("hostname".to_string())
        );
    }

    #[test]
    fn group_into_config_map_is_ok() {
        let config_map: ConfigMap = Group {
            enable: Some(true),
            enable_sni: Some(true),
            name: "name".to_string(),
            sni_override: Some("group_sni".to_string()),
            dns: vec![Dns {
                enable: Some(true),
                enable_sni: Some(true),
                hostname: "hostname".to_string(),
                sni_override: Some("sni".to_string()),
                address: Some("1.1.1.1".to_string()),
            }],
        }
        .into();
        assert_eq!(
            config_map.get("hostname").unwrap().sni,
            Some("group_sni".to_string())
        )
    }
}
