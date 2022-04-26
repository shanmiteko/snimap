use serde_derive::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Config {
    enable: Option<bool>,
    enable_sni: Option<bool>,
    group: Vec<Group>,
}

impl Config {
    pub fn group(&mut self) -> Option<&mut Vec<Group>> {
        if let Some(enable) = self.enable.or(Some(true)) {
            if enable && !self.group.is_empty() {
                return Some(self.group.as_mut());
            }
        }
        None
    }
}

#[derive(Deserialize, Serialize)]
pub struct Group {
    enable: Option<bool>,
    enable_sni: Option<bool>,
    name: String,
    dns: Vec<Dns>,
}

impl Group {
    pub fn new(name: &str, dns: Vec<Dns>) -> Self {
        Self {
            name: name.to_string(),
            dns,
            enable: None,
            enable_sni: None,
        }
    }

    pub fn dns(&mut self) -> Option<&mut Vec<Dns>> {
        if let Some(enable) = self.enable.or(Some(true)) {
            if enable && !self.dns.is_empty() {
                return Some(self.dns.as_mut());
            }
        }
        None
    }
}

#[derive(Deserialize, Serialize)]
pub struct Dns {
    enable: Option<bool>,
    enable_sni: Option<bool>,
    hostname: String,
    sni_override: Option<String>,
    address: Option<String>,
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

    pub fn hostname(&self) -> Option<&str> {
        if let Some(enable) = self.enable.or(Some(true)) {
            if enable && !self.hostname.is_empty() {
                return Some(&self.hostname);
            }
        }
        None
    }

    pub fn address(&self) -> &Option<String> {
        &self.address
    }

    pub fn address_mut(&mut self, address: String) {
        self.address = Some(address);
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
                        "wikipedia.org",
                        "zh.wikipedia.org",
                        "en.wikipedia.org",
                        "wikimedia.org",
                        "login.wikimedia.org",
                        "upload.wikimedia.org",
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
    use super::Config;

    #[test]
    fn config_default_is_ok() {
        toml::to_string_pretty(&Config::default()).unwrap();
    }
}
