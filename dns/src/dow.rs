use std::{marker::PhantomData, net::IpAddr};

use reqwest::RequestBuilder;
use serde::de;

use crate::resolver::Lookup;

pub trait DoWQuery {
    fn hostname(&self, hostname: &str) -> RequestBuilder;
}

pub trait DoWReply: for<'a> de::Deserialize<'a> {
    fn ip(self) -> Option<IpAddr>;
}

pub struct DnSpider<Query, Reply>(Query, PhantomData<Reply>);

impl<Query: Default, Reply> Default for DnSpider<Query, Reply> {
    fn default() -> Self {
        Self(Query::default(), PhantomData)
    }
}

#[async_trait::async_trait]
impl<Query, Reply> Lookup for DnSpider<Query, Reply>
where
    Query: DoWQuery + Send + Sync,
    Reply: DoWReply + Send + Sync,
{
    async fn lookup(&self, hostname: &str) -> Option<IpAddr> {
        self.0
            .hostname(hostname)
            .send()
            .await
            .ok()?
            .json::<Reply>()
            .await
            .ok()?
            .ip()
    }
}
