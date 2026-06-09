use std::{
    io::Result,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};
use tokio_socks::tcp::Socks5Stream;

pub enum Stream {
    Direct(TcpStream),
    Socks5(Socks5Stream<TcpStream>),
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Stream::Direct(s) => Pin::new(s).poll_read(cx, buf),
            Stream::Socks5(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        match self.get_mut() {
            Stream::Direct(s) => Pin::new(s).poll_write(cx, buf),
            Stream::Socks5(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        match self.get_mut() {
            Stream::Direct(s) => Pin::new(s).poll_flush(cx),
            Stream::Socks5(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        match self.get_mut() {
            Stream::Direct(s) => Pin::new(s).poll_shutdown(cx),
            Stream::Socks5(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}
