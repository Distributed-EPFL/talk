use crate::{
    crypto::primitives::sign::PublicKey,
    net::{Listener, SecureConnection, SecureReceiver, SecureSender},
    sync::fuse::{Fuse, Mikado, Relay},
    unicast::{
        Acknowledger, Message as UnicastMessage, ReceiverSettings, Response,
    },
};

use doomstack::{here, Doom, ResultExt, Top};

use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver as TokioReceiver, Sender as TokioSender};

type MessageInlet<Message> = TokioSender<(PublicKey, Message, Acknowledger)>;
type MessageOutlet<Message> = TokioReceiver<(PublicKey, Message, Acknowledger)>;

type ResponseInlet = TokioSender<Response>;
type ResponseOutlet = TokioReceiver<Response>;

pub struct Receiver<Message: UnicastMessage> {
    message_outlet: MessageOutlet<Message>,
    _fuse: Fuse,
}

#[derive(Doom)]
enum ListenError {
    #[doom(description("`listen` interrupted"))]
    ListenInterrupted,
}

#[derive(Doom)]
enum DriveInError {
    #[doom(description("`drive_in` interrupted"))]
    DriveInInterrupted,
    #[doom(description("Connection error"))]
    ConnectionError,
}

#[derive(Doom)]
enum DriveOutError {
    #[doom(description("`drive_out` interrupted"))]
    DriveOutInterrupted,
    #[doom(description("Connection error"))]
    ConnectionError,
}

impl<Message> Receiver<Message>
where
    Message: UnicastMessage,
{
    pub fn new<L>(listener: L, settings: ReceiverSettings) -> Self
    where
        L: Listener,
    {
        let (message_inlet, message_outlet) =
            mpsc::channel(settings.message_channel_capacity);

        let fuse = Fuse::new();
        let relay = fuse.relay();

        tokio::spawn(async move {
            let _ = Receiver::listen(listener, message_inlet, settings, relay)
                .await;
        });

        Receiver {
            message_outlet,
            _fuse: fuse,
        }
    }

    async fn listen<L>(
        mut listener: L,
        message_inlet: MessageInlet<Message>,
        settings: ReceiverSettings,
        mut relay: Relay,
    ) -> Result<(), Top<ListenError>>
    where
        L: Listener,
    {
        let fuse = Fuse::new();

        loop {
            if let Ok((remote, connection)) = relay
                .map(listener.accept())
                .await
                .pot(ListenError::ListenInterrupted, here!())?
            {
                let message_inlet = message_inlet.clone();
                let settings = settings.clone();
                let relay = fuse.relay();

                Receiver::spawn(
                    remote,
                    connection,
                    message_inlet,
                    settings,
                    relay,
                );
            }
        }
    }

    fn spawn(
        remote: PublicKey,
        connection: SecureConnection,
        message_inlet: MessageInlet<Message>,
        settings: ReceiverSettings,
        relay: Relay,
    ) {
        let (sender, receiver) = connection.split();

        let (response_inlet, response_outlet) =
            mpsc::channel::<Response>(settings.response_channel_capacity);

        let mikado_in = Mikado::new();
        let mikado_out = mikado_in.try_clone().unwrap();

        mikado_in.depend(relay);

        tokio::spawn(async move {
            let _ = Receiver::<Message>::drive_in(
                remote,
                receiver,
                message_inlet,
                response_inlet,
                mikado_in,
            )
            .await;
        });

        tokio::spawn(async move {
            let _ = Receiver::<Message>::drive_out(
                sender,
                response_outlet,
                mikado_out,
            )
            .await;
        });
    }

    async fn drive_in(
        remote: PublicKey,
        mut receiver: SecureReceiver,
        message_inlet: MessageInlet<Message>,
        response_inlet: ResponseInlet,
        mut mikado: Mikado,
    ) -> Result<(), Top<DriveInError>> {
        for sequence in 0..u32::MAX {
            let message: Message = mikado
                .map(receiver.receive())
                .await
                .pot(DriveInError::DriveInInterrupted, here!())?
                .pot(DriveInError::ConnectionError, here!())?;

            let acknowledger =
                Acknowledger::new(sequence, response_inlet.clone());

            let _ = message_inlet.try_send((remote, message, acknowledger));
        }

        Ok(())
    }

    async fn drive_out(
        mut sender: SecureSender,
        mut response_outlet: ResponseOutlet,
        mut mikado: Mikado,
    ) -> Result<(), Top<DriveOutError>> {
        loop {
            if let Some(response) = mikado
                .map(response_outlet.recv())
                .await
                .pot(DriveOutError::DriveOutInterrupted, here!())?
            {
                mikado
                    .map(sender.send(&response))
                    .await
                    .pot(DriveOutError::DriveOutInterrupted, here!())?
                    .pot(DriveOutError::ConnectionError, here!())?;
            }
        }
    }
}
