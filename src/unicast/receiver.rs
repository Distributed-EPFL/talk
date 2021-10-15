use crate::{
    crypto::primitives::sign::PublicKey,
    net::{Listener, SecureConnection, SecureSender},
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
#[doom(description("<Placeholder `ReceiverError`>"))]
pub struct ReceiverError;

#[derive(Doom)]
enum ListenError {
    #[doom(description("`listen` interrupted"))]
    ListenInterrupted,
}

#[derive(Doom)]
enum ServeError {
    #[doom(description("`serve` interrupted"))]
    ServeInterrupted,
    #[doom(description("Connection error"))]
    ConnectionError,
}

#[derive(Doom)]
enum AcknowledgeError {
    #[doom(description("`acknowledge` interrupted"))]
    AcknowledgeInterrupted,
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

                tokio::spawn(async move {
                    let _ = Receiver::serve(
                        remote,
                        connection,
                        message_inlet,
                        settings,
                        relay,
                    )
                    .await;
                });
            }
        }
    }

    async fn serve(
        remote: PublicKey,
        connection: SecureConnection,
        message_inlet: MessageInlet<Message>,
        settings: ReceiverSettings,
        relay: Relay,
    ) -> Result<(), Top<ServeError>> {
        let (sender, mut receiver) = connection.split();

        let (response_inlet, response_outlet) =
            mpsc::channel::<Response>(settings.response_channel_capacity);

        let mut mikado = Mikado::new();

        {
            let mikado = mikado.try_clone().unwrap();

            tokio::spawn(async move {
                let _ = Receiver::<Message>::acknowledge(
                    sender,
                    response_outlet,
                    mikado,
                )
                .await;
            });
        }

        mikado.depend(relay);

        for sequence in 0..u32::MAX {
            let message: Message = mikado
                .map(receiver.receive())
                .await
                .pot(ServeError::ServeInterrupted, here!())?
                .pot(ServeError::ConnectionError, here!())?;

            let acknowledger =
                Acknowledger::new(sequence, response_inlet.clone());

            let _ = message_inlet.try_send((remote, message, acknowledger));
        }

        Ok(())
    }

    async fn acknowledge(
        mut sender: SecureSender,
        mut response_outlet: ResponseOutlet,
        mut mikado: Mikado,
    ) -> Result<(), Top<AcknowledgeError>> {
        loop {
            if let Some(response) = mikado
                .map(response_outlet.recv())
                .await
                .pot(AcknowledgeError::AcknowledgeInterrupted, here!())?
            {
                mikado
                    .map(sender.send(&response))
                    .await
                    .pot(AcknowledgeError::AcknowledgeInterrupted, here!())?
                    .pot(AcknowledgeError::ConnectionError, here!())?;
            }
        }
    }
}
