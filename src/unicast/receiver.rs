use crate::{
    crypto::Identity,
    net::{Listener, Message as NetMessage, SecureConnection, SecureReceiver, SecureSender},
    sync::fuse::Fuse,
    unicast::{Acknowledgement, Acknowledger, ReceiverSettings, Request, Response},
};

use doomstack::{here, Doom, ResultExt, Top};

use tokio::sync::{
    mpsc,
    mpsc::{Receiver as TokioReceiver, Sender as TokioSender},
};

type MessageInlet<Message> = TokioSender<(Identity, Message, Acknowledger)>;
type MessageOutlet<Message> = TokioReceiver<(Identity, Message, Acknowledger)>;

type ResponseInlet = TokioSender<Response>;
type ResponseOutlet = TokioReceiver<Response>;

pub struct Receiver<Message: NetMessage> {
    message_outlet: MessageOutlet<Message>,
    _fuse: Fuse,
}

#[derive(Doom)]
enum ServeError {
    #[doom(description("`drive_in` failed"))]
    DriveInFailed,
    #[doom(description("`drive_out` failed"))]
    DriveOutFailed,
}

#[derive(Doom)]
enum DriveInError {
    #[doom(description("Connection error"))]
    ConnectionError,
}

#[derive(Doom)]
enum DriveOutError {
    #[doom(description("Connection error"))]
    ConnectionError,
}

impl<Message> Receiver<Message>
where
    Message: NetMessage,
{
    pub fn new<L>(listener: L, settings: ReceiverSettings) -> Self
    where
        L: Listener,
    {
        let (message_inlet, message_outlet) = mpsc::channel(settings.message_channel_capacity);

        let fuse = Fuse::new();

        fuse.spawn(async move {
            let _ = Receiver::listen(listener, message_inlet, settings).await;
        });

        Receiver {
            message_outlet,
            _fuse: fuse,
        }
    }

    pub async fn receive(&mut self) -> (Identity, Message, Acknowledger) {
        // This cannot fail, as `message_inlet` is held by `listen` until
        // `self._fuse` is dropped along with `self`: if `recv()` failed,
        // one could not call `receive()` in the first place
        self.message_outlet.recv().await.unwrap()
    }

    async fn listen<L>(
        mut listener: L,
        message_inlet: MessageInlet<Message>,
        settings: ReceiverSettings,
    ) where
        L: Listener,
    {
        let fuse = Fuse::new();

        loop {
            if let Ok((remote, connection)) = listener.accept().await {
                let message_inlet = message_inlet.clone();
                let settings = settings.clone();

                fuse.spawn(async move {
                    let _ = Receiver::serve(remote, connection, message_inlet, settings).await;
                });
            }
        }
    }

    async fn serve(
        remote: Identity,
        connection: SecureConnection,
        message_inlet: MessageInlet<Message>,
        settings: ReceiverSettings,
    ) -> Result<(), Top<ServeError>> {
        let (sender, receiver) = connection.split();

        let (response_inlet, response_outlet) =
            mpsc::channel::<Response>(settings.response_channel_capacity);

        let result = tokio::try_join!(
            async {
                Receiver::<Message>::drive_in(remote, receiver, message_inlet, response_inlet)
                    .await
                    .pot(ServeError::DriveInFailed, here!())
            },
            async {
                Receiver::<Message>::drive_out(sender, response_outlet)
                    .await
                    .pot(ServeError::DriveOutFailed, here!())
            }
        );

        result.map(|_| ())
    }

    async fn drive_in(
        remote: Identity,
        mut receiver: SecureReceiver,
        message_inlet: MessageInlet<Message>,
        response_inlet: ResponseInlet,
    ) -> Result<(), Top<DriveInError>> {
        for sequence in 0..u32::MAX {
            let request: Request<Message> = receiver
                .receive()
                .await
                .pot(DriveInError::ConnectionError, here!())?;

            match request {
                Request::Message(message) => {
                    let acknowledger = Acknowledger::new(sequence, response_inlet.clone());

                    let _ = message_inlet.try_send((remote, message, acknowledger));
                }
                Request::KeepAlive => {
                    let _ = response_inlet
                        .try_send(Response::Acknowledgement(sequence, Acknowledgement::Weak));
                }
            }
        }

        Ok(())
    }

    async fn drive_out(
        mut sender: SecureSender,
        mut response_outlet: ResponseOutlet,
    ) -> Result<(), Top<DriveOutError>> {
        loop {
            if let Some(response) = response_outlet.recv().await {
                sender
                    .send(&response)
                    .await
                    .pot(DriveOutError::ConnectionError, here!())?;
            }
        }
    }
}
