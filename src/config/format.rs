use std::collections::HashMap;

use serde_derive::{Deserialize, Serialize};

type Hostname = String;
type Sni = Option<Hostname>;

pub struct SniMap(HashMap<Hostname, Sni>);

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
    dnses: Vec<Dns>,
}

#[derive(Deserialize, Serialize)]
pub struct Dns {
    enable: Option<bool>,
    enable_sni: Option<bool>,
    hostname: String,
    sni: Option<String>,
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
        self.enable_sni.unwrap_or(true)
    }
}

impl Switchable for Group {
    fn enable(&self) -> bool {
        self.enable.unwrap_or(true)
    }

    fn enable_sni(&self) -> bool {
        self.enable_sni.unwrap_or(true)
    }
}

impl Switchable for Dns {
    fn enable(&self) -> bool {
        self.enable.unwrap_or(true)
    }

    fn enable_sni(&self) -> bool {
        self.enable_sni.unwrap_or(true)
    }
}

impl Config {
    pub fn new(groups: Vec<Group>) -> Self {
        Self {
            enable: None,
            enable_sni: None,
            groups,
        }
    }
}

impl Group {
    pub fn new(name: &str, enable_sni: Option<bool>, sni: Option<&str>, dnses: Vec<Dns>) -> Self {
        Self {
            name: name.to_string(),
            enable: None,
            enable_sni,
            sni: sni.map(ToString::to_string),
            dnses,
        }
    }
}

impl Dns {
    pub fn new(hostname: &str) -> Self {
        Self {
            enable: None,
            enable_sni: None,
            hostname: hostname.to_string(),
            sni: None,
        }
    }
}

impl SniMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn hostnames(&self) -> Vec<&str> {
        self.0.keys().map(|s| s.as_str()).collect()
    }

    pub fn get(&self, hostname: &str) -> Option<&Sni> {
        self.0.get(hostname)
    }

    pub fn insert(&mut self, k: Hostname, v: Sni) {
        self.0.insert(k, v);
    }

    pub fn merge<T: Into<SniMap>>(&mut self, other: T) {
        other
            .into()
            .0
            .into_iter()
            .for_each(|(k, v)| self.insert(k, v))
    }
}

impl From<Dns> for SniMap {
    fn from(dns: Dns) -> Self {
        let mut sni_map = SniMap::new();
        if dns.enable() {
            let enable_sni = dns.enable_sni();
            let Dns { hostname, sni, .. } = dns;
            sni_map.insert(
                hostname.clone(),
                enable_sni.then_some(sni.unwrap_or(hostname)),
            );
        }
        sni_map
    }
}

impl From<Group> for SniMap {
    fn from(group: Group) -> Self {
        let mut sni_map = SniMap::new();
        if group.enable() {
            let enable_sni = group.enable_sni();
            let Group {
                dnses: dns, sni, ..
            } = group;
            dns.into_iter().for_each(|mut d: Dns| {
                if enable_sni {
                    if sni.is_some() {
                        d.sni = sni.clone();
                    }
                } else {
                    d.enable_sni = Some(false);
                    d.sni = None;
                }
                sni_map.merge(d);
            });
        }
        sni_map
    }
}

impl From<Config> for SniMap {
    fn from(config: Config) -> Self {
        let mut sni_map = SniMap::new();
        if config.enable() {
            let enable_sni = config.enable_sni();
            config.groups.into_iter().for_each(|mut g: Group| {
                if !enable_sni {
                    g.enable_sni = Some(false);
                    g.sni = None;
                }
                sni_map.merge(g);
            });
        }
        sni_map
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new(vec![
            Group::new(
                "Duckduckgo",
                None,
                None,
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
                None,
                None,
                [
                    "avatars.githubusercontent.com",
                    "avatars0.githubusercontent.com",
                    "avatars1.githubusercontent.com",
                    "avatars2.githubusercontent.com",
                    "avatars3.githubusercontent.com",
                    "camo.githubusercontent.com",
                    "cloud.githubusercontent.com",
                    "gist.github.com",
                    "gist.githubusercontent.com",
                    "github.com",
                    "github.githubassets.com",
                    "raw.githubusercontent.com",
                    "user-images.githubusercontent.com",
                ]
                .into_iter()
                .map(Dns::new)
                .collect(),
            ),
            Group::new(
                "OneDrive",
                None,
                None,
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
                Some(false),
                None,
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
                "Pixiv",
                None,
                None,
                vec![
                    Dns {
                        enable: None,
                        enable_sni: None,
                        hostname: "pixiv.net".to_string(),
                        sni: Some("www.fanbox.cc".to_string()),
                    },
                    Dns {
                        enable: None,
                        enable_sni: None,
                        hostname: "www.pixiv.net".to_string(),
                        sni: Some("www.fanbox.cc".to_string()),
                    },
                    Dns {
                        enable: None,
                        enable_sni: None,
                        hostname: "accounts.pixiv.net".to_string(),
                        sni: Some("www.fanbox.cc".to_string()),
                    },
                    Dns {
                        enable: None,
                        enable_sni: Some(false),
                        hostname: "i.pximg.net".to_string(),
                        sni: Some("s.pximg.net".to_string()),
                    },
                ],
            ),
            Group::new(
                "Twich",
                None,
                None,
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
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, Dns, Group, SniMap};

    #[test]
    fn config_default() {
        toml::to_string_pretty(&Config::default()).unwrap();
    }

    #[test]
    fn dns_into_sni_map() {
        let sni_map: SniMap = Dns {
            enable: Some(false),
            enable_sni: Some(false),
            hostname: "hostname".to_string(),
            sni: Some("sni".to_string()),
        }
        .into();
        assert_eq!(sni_map.0.len(), 0, "1");
        assert_eq!(sni_map.get("hostname"), None);
        assert_eq!(sni_map.get("hostname"), None);

        let sni_map: SniMap = Dns {
            enable: Some(true),
            enable_sni: Some(false),
            hostname: "hostname".to_string(),
            sni: Some("sni".to_string()),
        }
        .into();
        assert_eq!(sni_map.get("hostname"), Some(&None));

        let sni_map: SniMap = Dns {
            enable: Some(true),
            enable_sni: Some(true),
            hostname: "hostname".to_string(),
            sni: Some("sni".to_string()),
        }
        .into();
        assert_eq!(sni_map.get("hostname"), Some(&Some("sni".to_string())));

        let sni_map: SniMap = Dns {
            enable: Some(true),
            enable_sni: Some(true),
            hostname: "hostname".to_string(),
            sni: None,
        }
        .into();
        assert_eq!(sni_map.get("hostname"), Some(&Some("hostname".to_string())));
    }

    #[test]
    fn group_into_config_map() {
        let sni_map: SniMap = Group {
            enable: Some(true),
            enable_sni: Some(false),
            name: "name".to_string(),
            sni: Some("group_sni".to_string()),
            dnses: vec![Dns {
                enable: Some(true),
                enable_sni: Some(true),
                hostname: "hostname".to_string(),
                sni: Some("sni".to_string()),
            }],
        }
        .into();
        assert_eq!(sni_map.get("hostname"), Some(&None));
    }

    #[test]
    fn config_into_config_map() {
        let sni_map: SniMap = Config {
            enable: Some(true),
            enable_sni: Some(true),
            groups: vec![Group {
                enable: Some(true),
                enable_sni: Some(false),
                name: "name".to_string(),
                sni: Some("group_sni".to_string()),
                dnses: vec![Dns {
                    enable: Some(true),
                    enable_sni: Some(true),
                    hostname: "hostname".to_string(),
                    sni: Some("sni".to_string()),
                }],
            }],
        }
        .into();
        assert_eq!(sni_map.get("hostname"), Some(&None));
    }
}
