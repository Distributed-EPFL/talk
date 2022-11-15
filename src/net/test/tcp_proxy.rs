use crate::sync::fuse::Fuse;
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::{
    io,
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::{
        mpsc,
        mpsc::{Receiver as MpscReceiver, Sender as MpscSender},
        watch,
        watch::{Receiver as WatchReceiver, Sender as WatchSender},
        RwLock,
    },
};

type StateInlet = WatchSender<State>;
type StateOutlet = WatchReceiver<State>;

type ResetInlet = MpscSender<()>;
type ResetOutlet = MpscReceiver<()>;

pub struct TcpProxy {
    address: SocketAddr,
    state_inlet: StateInlet,
    reset_inlet: ResetInlet,
    on_lock: Arc<RwLock<()>>,
    off_lock: Arc<RwLock<()>>,
    _fuse: Fuse,
}

#[derive(Clone, Copy)]
enum State {
    On,
    Off,
}

impl TcpProxy {
    pub async fn new<A>(server: A) -> Self
    where
        A: 'static + Send + Sync + Clone + ToSocketAddrs,
    {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();

        let address = listener.local_addr().unwrap();

        let (state_inlet, state_outlet) = watch::channel(State::On);
        let (reset_inlet, reset_outlet) = mpsc::channel(32); // TODO: Add settings

        let on_lock = Arc::new(RwLock::new(()));
        let off_lock = Arc::new(RwLock::new(()));

        let fuse = Fuse::new();

        {
            let on_lock = on_lock.clone();
            let off_lock = off_lock.clone();

            fuse.spawn(async move {
                let _ = TcpProxy::listen(
                    listener,
                    server,
                    state_outlet,
                    reset_outlet,
                    on_lock,
                    off_lock,
                )
                .await;
            });
        }

        Self {
            address,
            state_inlet,
            reset_inlet,
            on_lock,
            off_lock,
            _fuse: fuse,
        }
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub async fn start(&mut self) {
        let _ = self.state_inlet.send(State::On);
        let _ = self.off_lock.write().await;
    }

    pub async fn stop(&mut self) {
        let _ = self.state_inlet.send(State::Off);
        let _ = self.on_lock.write().await;
    }

    pub async fn reset(&mut self) {
        let _ = self.reset_inlet.send(()).await;
    }

    async fn listen<A>(
        listener: TcpListener,
        server: A,
        state_outlet: StateOutlet,
        mut reset_outlet: ResetOutlet,
        on_lock: Arc<RwLock<()>>,
        off_lock: Arc<RwLock<()>>,
    ) where
        A: 'static + Send + Sync + Clone + ToSocketAddrs,
    {
        let mut fuse = Fuse::new();

        loop {
            tokio::select! {
                biased;

                _ = reset_outlet.recv() => {
                    fuse = Fuse::new()
                }

                Ok((stream, _)) = listener.accept() => {
                    let server = server.clone();

                    let on_lock = on_lock.clone();
                    let off_lock = off_lock.clone();

                    let state_outlet = state_outlet.clone();

                    fuse.spawn(async move {
                        let _ = TcpProxy::forward(
                            stream,
                            server,
                            state_outlet,
                            on_lock,
                            off_lock,
                        )
                        .await;
                    });
                }
            }
        }
    }

    async fn forward<A>(
        mut client: TcpStream,
        server: A,
        mut state_outlet: StateOutlet,
        on_lock: Arc<RwLock<()>>,
        off_lock: Arc<RwLock<()>>,
    ) -> Result<(), io::Error>
    where
        A: 'static + Send + Sync + Clone + ToSocketAddrs,
    {
        let mut server = TcpStream::connect(server).await.unwrap();

        let (mut client_read, mut client_write) = client.split();
        let (mut server_read, mut server_write) = server.split();

        let mut client_buffer = [0u8; 1024];
        let mut server_buffer = [0u8; 1024];

        let mut _on_guard = Some(on_lock.read().await);
        let mut _off_guard = Some(off_lock.read().await);

        loop {
            let state = *state_outlet.borrow_and_update();

            match state {
                State::On => {
                    _off_guard = None;

                    loop {
                        tokio::select! {
                            biased;

                            _ = state_outlet.changed() => {
                                break;
                            },

                            result = client_read.read(&mut client_buffer) => {
                                let written = result?;
                                server_write.write_all(&client_buffer[0..written]).await?;
                            }

                            result = server_read.read(&mut server_buffer) => {
                                let written = result?;
                                client_write.write_all(&server_buffer[0..written]).await?;
                            }
                        }
                    }

                    _off_guard = Some(off_lock.read().await);
                }
                State::Off => {
                    _on_guard = None;

                    let _ = state_outlet.changed().await;

                    _on_guard = Some(on_lock.read().await);
                }
            };
        }
    }
}
