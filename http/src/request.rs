use std::{fmt, io::IoSlice};

use tokio::io::AsyncWriteExt;

use crate::by;

use super::consts::*;

/// [HTTP Request](https://www.rfc-editor.org/rfc/rfc2616#section-5)
pub struct Request<'a> {
    pub method: &'a [u8],
    pub uri: &'a [u8],
    pub version: &'a [u8],
    pub headers: Vec<&'a [u8]>,
    pub body: &'a [u8],
}

impl fmt::Debug for Request<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpRequest")
            .field(
                "method",
                &self.method.iter().map(|b| *b as char).collect::<String>(),
            )
            .field(
                "uri",
                &self.uri.iter().map(|b| *b as char).collect::<String>(),
            )
            .field(
                "version",
                &self.version.iter().map(|b| *b as char).collect::<String>(),
            )
            .field(
                "headers",
                &self
                    .headers
                    .iter()
                    .map(|h| h.iter().map(|b| *b as char).collect::<String>())
                    .collect::<Vec<String>>(),
            )
            .field(
                "body",
                &self
                    .body
                    .iter()
                    .map(|b| {
                        if b.is_ascii_graphic() {
                            *b as char
                        } else {
                            '.'
                        }
                    })
                    .collect::<String>(),
            )
            .finish()
    }
}

impl Request<'_> {
    pub fn from_bytes(data: &[u8]) -> Option<Request> {
        let mut data_split = data.split(by!(&CR));

        let request_line = data_split.next()?;
        let mut request_line_split = request_line.split(by!(&SP));
        let method = request_line_split.next()?;
        let uri = request_line_split.next()?;
        let version = request_line_split.next()?;

        let _ = version.starts_with(HTTP).then_some(0)?;

        let mut headers = Vec::new();
        let mut has_crlf = false;
        for header in data_split.by_ref() {
            if header.len() == 1 && header[0] == LF {
                has_crlf = true;
                break;
            }
            headers.push(&header[1..])
        }

        let _ = has_crlf.then_some(0)?;

        let body = &data_split.next()?[1..];
        Some(Request {
            method,
            uri,
            version,
            headers,
            body,
        })
    }

    pub fn headers_filter<F>(mut self, pred: F) -> Self
    where
        F: Fn(&[u8]) -> bool,
    {
        let mut headers = Vec::with_capacity(self.headers.len());
        for header in &self.headers {
            if pred(header) {
                headers.push(*header)
            }
        }
        self.headers = headers;
        self
    }

    pub async fn send_to<W>(&self, to: &mut W) -> std::io::Result<usize>
    where
        W: AsyncWriteExt + Unpin,
    {
        to.write_vectored(
            &[
                self.method,
                &[SP],
                self.uri,
                &[SP],
                self.version,
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
