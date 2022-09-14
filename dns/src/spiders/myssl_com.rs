//! https://myssl.com/dns_check.html

use reqwest::RequestBuilder;
use serde_derive::Deserialize;

use crate::dow::{DnSpider, DoWQuery, DoWReply};

use super::client::CLIENT;

pub type MysslCom = DnSpider<MysslComQuery, MysslComReply>;

#[derive(Default)]
pub struct MysslComQuery;

impl DoWQuery for MysslComQuery {
    fn hostname(&self, hostname: &str) -> RequestBuilder {
        CLIENT.get(format!(
            "https://myssl.com/api/v1/tools/dns_query?qtype=1&host={}&qmode=-1",
            hostname
        ))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MysslComReply {
    #[serde(skip)]
    pub code: i64,
    pub data: Data,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    #[serde(rename = "01")]
    pub n01: Vec<Dns>,
    #[serde(skip)]
    #[serde(rename = "852")]
    pub n852: Vec<Dns>,
    #[serde(skip)]
    #[serde(rename = "86")]
    pub n86: Vec<Dns>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dns {
    pub answer: Answer,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    #[serde(skip)]
    #[serde(rename = "time_consume")]
    pub time_consume: String,
    pub records: Option<Vec<Record>>,
    #[serde(skip)]
    pub error: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    #[serde(skip)]
    pub ttl: i64,
    pub value: String,
    #[serde(skip)]
    #[serde(rename = "ip_location")]
    pub ip_location: String,
}

impl DoWReply for MysslComReply {
    fn ip(mut self) -> Option<std::net::IpAddr> {
        self.data
            .n01
            .pop()?
            .answer
            .records?
            .get(0)?
            .value
            .parse()
            .ok()
    }
}
