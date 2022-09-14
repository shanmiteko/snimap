use std::fmt;

use thiserror::Error;

pub type ProxyResult<T> = Result<T, ProxyError>;

#[derive(Error, Debug, Default)]
pub struct ProxyError {
    kind: ProxyErrorKind,
    upstream: Option<String>,
    downstream: Option<String>,
    from: Option<String>,
    context: Option<String>,
}

#[derive(Debug)]
pub enum ProxyErrorKind {
    NotHttpRequest,
    InvalidHost,
    HostNotFound,
    ConnectIo,
    ConnectTimeout,
    ReadIo,
    ReadTimeout,
    Other,
}

impl Default for ProxyErrorKind {
    fn default() -> Self {
        Self::Other
    }
}

impl fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} -> {}: {}: {}",
            self.downstream.as_ref().unwrap_or(&"x".to_string()),
            self.upstream.as_ref().unwrap_or(&"x".to_string()),
            self.context.as_ref().unwrap_or(&"?".to_string()),
            self.from.as_ref().unwrap_or(&"?".to_string()),
        )
    }
}

impl ProxyError {
    pub fn new(kind: ProxyErrorKind) -> Self {
        Self {
            kind,
            ..Default::default()
        }
    }

    pub fn kind(&self) -> &ProxyErrorKind {
        &self.kind
    }

    pub fn upstream<T: ToString>(mut self, upstream: T) -> Self {
        self.upstream = Some(upstream.to_string());
        self
    }

    pub fn downstream<T: ToString>(mut self, downstream: T) -> Self {
        self.downstream = Some(downstream.to_string());
        self
    }

    pub fn from<T: ToString>(mut self, from: T) -> Self {
        self.from = Some(from.to_string());
        self
    }

    pub fn context<T: ToString>(mut self, context: T) -> Self {
        self.context = Some(context.to_string());
        self
    }
}
