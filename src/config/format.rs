use std::collections::{HashMap, HashSet};

use serde_derive::{Deserialize, Serialize};

type Hostname = String;

#[derive(PartialEq, Eq, Debug)]
pub enum Sni {
    Disable,
    Override(Hostname),
    Remain(Hostname),
}

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
    mappings: Vec<Mapping>,
}

#[derive(Deserialize, Serialize)]
pub struct Mapping {
    enable: Option<bool>,
    enable_sni: Option<bool>,
    hostname: String,
    sni: Option<String>,
}

pub trait Switchable: Sized {
    fn enable(&self) -> Option<bool>;
    fn enable_sni(&self) -> Option<bool>;
    fn enable_mut(&mut self) -> &mut Option<bool>;
    fn enable_sni_mut(&mut self) -> &mut Option<bool>;
    fn enabled(&self) -> bool {
        self.enable().unwrap_or(true)
    }
    fn enabled_sni(&self) -> bool {
        self.enable_sni().unwrap_or(true)
    }
    fn disable_sni(mut self) -> Self {
        *self.enable_sni_mut() = Some(false);
        self
    }
}

macro_rules! impl_switchable {
    ($($i:ident),*) => {
        $(impl Switchable for $i {
            fn enable(&self) -> Option<bool> {
                self.enable
            }

            fn enable_sni(&self) -> Option<bool> {
                self.enable_sni
            }

            fn enable_mut(&mut self) -> &mut Option<bool> {
                &mut self.enable
            }

            fn enable_sni_mut(&mut self) -> &mut Option<bool> {
                &mut self.enable_sni
            }
        })*
    };
}
impl_switchable!(Config, Group, Mapping);

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
    pub fn new(name: &str, mappings: Vec<Mapping>) -> Self {
        Self {
            name: name.to_string(),
            enable: None,
            enable_sni: None,
            sni: None,
            mappings,
        }
    }
}

impl Mapping {
    pub fn new(hostname: &str) -> Self {
        Self {
            enable: None,
            enable_sni: None,
            hostname: hostname.to_string(),
            sni: None,
        }
    }

    pub fn override_sni(mut self, sni: &str) -> Self {
        self.sni = Some(sni.to_string());
        self
    }
}

impl SniMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn hostnames(&self) -> HashSet<&str> {
        self.0.keys().map(|s| s.as_str()).collect()
    }

    pub fn overrided_sni(&self) -> HashSet<&str> {
        self.0
            .values()
            .filter_map(|sni| match sni {
                Sni::Disable | Sni::Remain(_) => None,
                Sni::Override(host) => Some(host.as_str()),
            })
            .collect()
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

impl From<Mapping> for SniMap {
    fn from(dns: Mapping) -> Self {
        let mut snimap = SniMap::new();
        if dns.enabled() {
            let enable_sni = dns.enabled_sni();
            let Mapping { hostname, sni, .. } = dns;
            let sni = match enable_sni {
                true => match sni {
                    Some(sniname) if sniname == hostname => Sni::Remain(hostname.clone()),
                    Some(sniname) => Sni::Override(sniname),
                    None => Sni::Remain(hostname.clone()),
                },
                _ => Sni::Disable,
            };
            snimap.insert(hostname, sni)
        }
        snimap
    }
}

impl From<Group> for SniMap {
    fn from(group: Group) -> Self {
        let mut snimap = SniMap::new();
        if group.enabled() {
            let enable_sni = group.enabled_sni();
            let Group { mappings, sni, .. } = group;
            mappings.into_iter().for_each(|mut d: Mapping| {
                if enable_sni {
                    if sni.is_some() {
                        d.sni = sni.clone();
                    }
                } else {
                    d.enable_sni = Some(false);
                    d.sni = None;
                }
                snimap.merge(d);
            });
        }
        snimap
    }
}

impl From<Config> for SniMap {
    fn from(config: Config) -> Self {
        let mut snimap = SniMap::new();
        if config.enabled() {
            let enable_sni = config.enabled_sni();
            config.groups.into_iter().for_each(|mut g: Group| {
                if !enable_sni {
                    g.enable_sni = Some(false);
                    g.sni = None;
                }
                snimap.merge(g);
            });
        }
        snimap
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new(vec![
            Group::new(
                "Duckduckgo",
                [
                    "duck.com",
                    "duckduckgo.com",
                    "external-content.duckduckgo.com",
                    "links.duckduckgo.com",
                ]
                .into_iter()
                .map(Mapping::new)
                .collect(),
            ),
            Group::new(
                "Github",
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
                    "objects.githubusercontent.com",
                    "raw.githubusercontent.com",
                    "user-images.githubusercontent.com",
                ]
                .into_iter()
                .map(Mapping::new)
                .collect(),
            )
            .disable_sni(),
            Group::new(
                "OneDrive",
                [
                    "onedrive.com",
                    "api.onedrive.com",
                    "onedrive.live.com",
                    "skyapi.onedrive.live.com",
                ]
                .into_iter()
                .map(Mapping::new)
                .collect(),
            ),
            Group::new(
                "Wikipedia",
                [
                    "wikipedia.org",
                    "zh.wikipedia.org",
                    "en.wikipedia.org",
                    "wikimedia.org",
                    "login.wikimedia.org",
                    "upload.wikimedia.org",
                    "maps.wikimedia.org",
                ]
                .into_iter()
                .map(Mapping::new)
                .collect(),
            )
            .disable_sni(),
            Group::new(
                "Pixiv",
                vec![
                    Mapping::new("pixiv.net").override_sni("www.fanbox.cc"),
                    Mapping::new("www.pixiv.net").override_sni("www.fanbox.cc"),
                    Mapping::new("accounts.pixiv.net").override_sni("www.fanbox.cc"),
                    Mapping::new("i.pximg.net").override_sni("s.pximg.net"),
                ],
            ),
            Group::new(
                "Twich",
                [
                    "twitch.tv",
                    "www.twitch.tv",
                    "static.twitchcdn.net",
                    "gql.twitch.tv",
                    "passport.twitch.tv",
                ]
                .into_iter()
                .map(Mapping::new)
                .collect(),
            )
            .disable_sni(),
        ])
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Sni;

    use super::{Config, Group, Mapping, SniMap};

    #[test]
    fn config_default() {
        toml::to_string_pretty(&Config::default()).unwrap();
    }

    #[test]
    fn dns_into_snimap() {
        let snimap: SniMap = Mapping {
            enable: Some(false),
            enable_sni: Some(false),
            hostname: "hostname".to_string(),
            sni: Some("sni".to_string()),
        }
        .into();
        assert_eq!(snimap.0.len(), 0, "1");
        assert_eq!(snimap.get("hostname"), None);
        assert_eq!(snimap.get("hostname"), None);

        let snimap: SniMap = Mapping {
            enable: Some(true),
            enable_sni: Some(false),
            hostname: "hostname".to_string(),
            sni: Some("sni".to_string()),
        }
        .into();
        assert_eq!(snimap.get("hostname"), Some(&Sni::Disable));

        let snimap: SniMap = Mapping {
            enable: Some(true),
            enable_sni: Some(true),
            hostname: "hostname".to_string(),
            sni: Some("sni".to_string()),
        }
        .into();
        assert_eq!(
            snimap.get("hostname"),
            Some(&Sni::Override("sni".to_string()))
        );

        let snimap: SniMap = Mapping {
            enable: Some(true),
            enable_sni: Some(true),
            hostname: "hostname".to_string(),
            sni: None,
        }
        .into();
        assert_eq!(
            snimap.get("hostname"),
            Some(&Sni::Remain("hostname".to_string()))
        );
    }

    #[test]
    fn group_into_config_map() {
        let snimap: SniMap = Group {
            enable: Some(true),
            enable_sni: Some(false),
            name: "name".to_string(),
            sni: Some("group_sni".to_string()),
            mappings: vec![Mapping {
                enable: Some(true),
                enable_sni: Some(true),
                hostname: "hostname".to_string(),
                sni: Some("sni".to_string()),
            }],
        }
        .into();
        assert_eq!(snimap.get("hostname"), Some(&Sni::Disable));
    }

    #[test]
    fn config_into_config_map() {
        let snimap: SniMap = Config {
            enable: Some(true),
            enable_sni: Some(true),
            groups: vec![Group {
                enable: Some(true),
                enable_sni: Some(false),
                name: "name".to_string(),
                sni: Some("group_sni".to_string()),
                mappings: vec![Mapping {
                    enable: Some(true),
                    enable_sni: Some(true),
                    hostname: "hostname".to_string(),
                    sni: Some("sni".to_string()),
                }],
            }],
        }
        .into();
        assert_eq!(snimap.get("hostname"), Some(&Sni::Disable));
    }
}
