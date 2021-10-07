use crate::{
    crypto::primitives::sign::PublicKey,
    net::{Listener, SecureConnection, SecureSender},
    sync::fuse::{Fuse, Relay},
    unicast::{
        errors::receiver::{
            acknowledge::{
                AcknowledgeError, ConnectionError as AcknowledgeConnectionError,
            },
            listen::{ListenError, ListenInterrupted},
            serve::{
                ConnectionError as ServeConnectionError, ServeError,
                ServeInterrupted,
            },
        },
        Acknowledger, Message as UnicastMessage, ReceiverSettings, Response,
    },
};

use snafu::ResultExt;

use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver as TokioReceiver, Sender as TokioSender};

type MessageInlet<Message> = TokioSender<(PublicKey, Message, Acknowledger)>;
type MessageOutlet<Message> = TokioReceiver<(PublicKey, Message, Acknowledger)>;

type ResponseInlet = TokioSender<Response>;
type ResponseOutlet = TokioReceiver<Response>;

pub struct Receiver<Message: UnicastMessage> {
    message_outlet: MessageOutlet<Message>,
    fuse: Fuse,
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
            fuse,
        }
    }

    async fn listen<L>(
        mut listener: L,
        message_inlet: MessageInlet<Message>,
        settings: ReceiverSettings,
        mut relay: Relay,
    ) -> Result<(), ListenError>
    where
        L: Listener,
    {
        let fuse = Fuse::new();

        loop {
            if let Ok((remote, connection)) = relay
                .map(listener.accept())
                .await
                .context(ListenInterrupted)?
            {
                let message_inlet = message_inlet.clone();
                let relay = fuse.relay();
                let settings = settings.clone();

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
        mut relay: Relay,
    ) -> Result<(), ServeError> {
        let (sender, mut receiver) = connection.split();

        let (response_inlet, response_outlet) =
            mpsc::channel::<Response>(settings.response_channel_capacity);

        tokio::spawn(async move {
            let _ = Receiver::<Message>::acknowledge(sender, response_outlet).await;
        });

        for sequence in 0..u32::MAX {
            let message: Message = relay
                .map(receiver.receive())
                .await
                .context(ServeInterrupted)?
                .context(ServeConnectionError)?;

            let acknowledger = Acknowledger::new(sequence, response_inlet.clone());
            let _ = message_inlet.send((remote, message, acknowledger)).await;
        }

        Ok(())
    }

    async fn acknowledge(
        mut sender: SecureSender,
        mut response_outlet: ResponseOutlet,
    ) -> Result<(), AcknowledgeError> {
        loop {
            match response_outlet.recv().await {
                Some(response) => {
                    sender
                        .send(&response)
                        .await
                        .context(AcknowledgeConnectionError)?;
                }
                None => return Ok(()),
            }
        }
    }
}
