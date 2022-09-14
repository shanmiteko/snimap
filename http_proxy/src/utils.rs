use std::{future::Future, time::Duration};

use bytes::BytesMut;
use log::trace;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::{ReadHalf, WriteHalf},
    time::{timeout, Timeout},
};

pub fn tokio_timeout<T>(secs: u64, future: T) -> Timeout<T>
where
    T: Future,
{
    timeout(Duration::from_secs(secs), future)
}

pub async fn read_until<'a: 'b, 'b, R, F>(
    incoming: &'b mut R,
    timeout: u64,
    buf: &'b mut BytesMut,
    pred: F,
) -> Result<std::io::Result<usize>, tokio::time::error::Elapsed>
where
    R: AsyncReadExt + Unpin,
    F: Fn(&[u8]) -> bool,
{
    let mut result = None;
    loop {
        if pred(&*buf) {
            break;
        }
        match tokio_timeout(timeout, incoming.read_buf(buf)).await {
            Ok(connect_state) => match connect_state {
                Ok(n) => {
                    result = Some(Ok(Ok(n)));
                    if n == 0 {
                        break;
                    }
                }
                Err(e) => {
                    result = Some(Ok(Err(e)));
                    break;
                }
            },
            Err(elapsed) => {
                result = Some(Err(elapsed));
                break;
            }
        }
    }
    result.unwrap()
}

pub async fn pipe<'a: 'b, 'b>(
    from: &'b mut ReadHalf<'a>,
    to: &'b mut WriteHalf<'a>,
) -> std::io::Result<usize> {
    let mut buf = [0u8; 1024];

    loop {
        trace!(
            "{:?} -> {:?} waiting for read ...",
            from.peer_addr(),
            to.peer_addr()
        );
        let num = from.read(&mut buf).await?;
        trace!(
            "{:?} -> {:?} {:?}",
            from.peer_addr(),
            to.peer_addr(),
            &buf[..num]
                .iter()
                .map(|b| if b.is_ascii_graphic() {
                    *b as char
                } else {
                    '.'
                })
                .collect::<String>(),
        );

        if num == 0 {
            to.shutdown().await?;
            break;
        }

        to.write_all(&buf[..num]).await?
    }

    Ok(0)
}
