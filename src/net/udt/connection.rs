use udt::{UdtSocket, UdtOpts, SocketFamily, SocketType, UdtError};
use libudt4_sys::{EASYNCRCV, EASYNCSND};
use crate::net::Socket;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, Error, ErrorKind};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

struct UdtConnection {
    socket: UdtSocket
}

impl UdtConnection {
    pub fn init() -> Self {
        let socket = UdtSocket::new(SocketFamily::AFInet, SocketType::Stream).unwrap();
        socket.setsockopt(UdtOpts::UDT_RCVSYN, false).unwrap();
        socket.setsockopt(UdtOpts::UDT_SNDSYN, false).unwrap();
        Self {
            socket
        }
    }
}

impl AsyncRead for UdtConnection {
    fn poll_read(
        self: Pin<&mut Self>, 
        cx: &mut Context<'_>, 
        buf: &mut ReadBuf<'_>
    ) -> Poll<tokio::io::Result<()>> {

        let res = (*self).socket.recv(buf.initialize_unfilled(), 1024);
    
        match res {
            Ok(_) => Poll::Ready(Ok(())),
            Err(UdtError{
                // no data available for read
                err_code: EASYNCRCV,
                ..
            }) => {
                let waker = cx.waker().clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    waker.wake();
                });
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(Error::new(ErrorKind::Other, e.err_msg)))
        }
        
    }
}

impl AsyncWrite for UdtConnection {
    fn poll_write(
        self: Pin<&mut Self>, 
        cx: &mut Context<'_>, 
        buf: &[u8]
    ) -> Poll<Result<usize, Error>> {
        let res = (*self).socket.send(buf);
        match res {
            Ok(n) => Poll::Ready(Ok(n as usize)),
            Err(UdtError{
                // no buffer available for sending
                err_code: EASYNCSND,
                ..
            }) => {
                let waker = cx.waker().clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    waker.wake();
                });
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(Error::new(ErrorKind::Other, e.err_msg)))
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>, 
        _cx: &mut Context<'_>
    ) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>, 
        _cx: &mut Context<'_>
    ) -> Poll<Result<(), Error>> {
        // Best effort shutdown
        match self.socket.close() {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e) => Poll::Ready(Err(Error::new(ErrorKind::Other, e.err_msg)))
        }
    }

}

impl Socket for UdtConnection {}
