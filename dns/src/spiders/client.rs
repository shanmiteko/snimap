use std::time::Duration;

use once_cell::sync::Lazy;
use reqwest::{Client, ClientBuilder};

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:104.0) Gecko/20100101 Firefox/104.0";

pub static CLIENT: Lazy<Client> = Lazy::new(|| {
    ClientBuilder::new()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
});
