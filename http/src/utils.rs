use subslice::SubsliceExt;

use crate::by;

use super::consts::*;

pub fn http_head_end(data: &[u8]) -> bool {
    data.rfind(CRLFCRLF).is_some()
}

pub fn extract_host(uri: &[u8]) -> Option<String> {
    let mut uri_split = uri.split(by!(&SLASH));
    let mut socket = uri_split.next()?;
    if let Some(authority) = uri_split.nth(1) {
        socket = authority.split(by!(&AT)).last()?;
    }
    let mut socket_split = socket.split(by!(&COLON));
    let hostname = socket_split.next()?;
    let port = socket_split.next().unwrap_or(b"80");
    Some(
        [hostname, &[COLON], port]
            .concat()
            .iter()
            .map(|b| *b as char)
            .collect::<String>(),
    )
}
