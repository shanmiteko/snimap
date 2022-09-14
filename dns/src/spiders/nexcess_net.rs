//! https://www.nexcess.net/web-tools/dns-checker/

use reqwest::RequestBuilder;
use serde_derive::Deserialize;

use crate::dow::{DnSpider, DoWQuery, DoWReply};

use super::client::CLIENT;

pub type NexcessNet = DnSpider<NexcessNetQuery, NexcessNetReply>;

#[derive(Default)]
pub struct NexcessNetQuery;

impl DoWQuery for NexcessNetQuery {
    fn hostname(&self, hostname: &str) -> RequestBuilder {
        CLIENT
            .post("https://tools.nexcess.net/dns-check")
            .body(format!("id=google-dns&type=A&hostname={}", hostname))
    }
}

#[derive(Deserialize)]
pub struct NexcessNetReply {
    #[serde(skip)]
    pub error: bool,
    pub data: Data,
    #[serde(skip)]
    pub status: i64,
}

#[derive(Deserialize)]
pub struct Data {
    #[serde(skip)]
    pub sid: String,
    pub result: Vec<String>,
}

impl DoWReply for NexcessNetReply {
    fn ip(self) -> Option<std::net::IpAddr> {
        self.data.result.get(0)?.parse().ok()
    }
}
