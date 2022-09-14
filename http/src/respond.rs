use std::io::IoSlice;

use tokio::io::AsyncWriteExt;

use crate::const_enum;

use super::consts::*;

const_enum! {
    pub enum Version: &'static [u8] {
        HTTP1_1 = b"HTTP/1.1",
    }
}

const_enum! {
    pub enum Status: &'static [u8] {
        Ok = b"200 OK",
        BadRequest = b"400 Bad Request",
        RequestTimeout = b"408 Request Timeout",
        BadGateway = b"502 Bad Gateway",
        GatewayTimeout = b"504 Gateway Timeout",
    }
}

pub struct RespondBuilder {
    version: Version,
    status: Status,
}

impl Default for RespondBuilder {
    /// Version::HTTP1_1
    ///
    /// Status::Ok
    fn default() -> Self {
        Self {
            version: Version::HTTP1_1,
            status: Status::Ok,
        }
    }
}

impl RespondBuilder {
    pub fn version(mut self, version: Version) -> Self {
        self.version = version;
        self
    }

    pub fn status(mut self, status: Status) -> Self {
        self.status = status;
        self
    }

    pub fn nobody(self) -> Respond<'static> {
        Respond {
            version: self.version.inner,
            status_reason: self.status.inner,
            headers: vec![],
            body: &[],
        }
    }
}

/// [Http Response](https://www.rfc-editor.org/rfc/rfc2616#section-6)
pub struct Respond<'a> {
    pub version: &'a [u8],
    pub status_reason: &'a [u8],
    pub headers: Vec<&'a [u8]>,
    pub body: &'a [u8],
}

impl Respond<'_> {
    pub async fn send_to<W>(&self, to: &mut W) -> std::io::Result<usize>
    where
        W: AsyncWriteExt + Unpin,
    {
        to.write_vectored(
            &[
                self.version,
                &[SP],
                self.status_reason,
                CRLF,
                &self.headers.join(&CRLF[..]),
                CRLF,
                self.body,
            ]
            .map(IoSlice::new),
        )
        .await
    }
}
