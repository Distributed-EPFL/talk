use crate::{
    crypto::primitives::sign::PublicKey,
    net::{Connector, SecureReceiver, SecureSender},
    sync::fuse::{Fuse, Mikado, Relay},
    unicast::{Acknowledgement, Message as UnicastMessage, Request, Response},
};

use doomstack::{here, Doom, ResultExt, Stack, Top};

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{Receiver as MpscReceiver, Sender as MpscSender};
use tokio::sync::oneshot;
use tokio::sync::oneshot::{
    Receiver as OneshotReceiver, Sender as OneshotSender,
};

use std::collections::HashMap;

type RequestInlet<Message> =
    MpscSender<(Request<Message>, AcknowledgementInlet)>;
type RequestOutlet<Message> =
    MpscReceiver<(Request<Message>, AcknowledgementInlet)>;

type AcknowledgementInlet =
    OneshotSender<Result<Acknowledgement, Top<CasterError>>>;
type AcknowledgementOutlet =
    OneshotReceiver<Result<Acknowledgement, Top<CasterError>>>;

pub(in crate::unicast) struct Caster<Message: UnicastMessage> {
    state: Arc<Mutex<State<Message>>>,
    _fuse: Fuse,
}

enum State<Message: UnicastMessage> {
    Running(RequestInlet<Message>),
    Terminated,
}

struct Database {
    acknowledgement_inlets: HashMap<u32, AcknowledgementInlet>,
}

pub(in crate::unicast) struct CasterTerminated<Message: UnicastMessage>(
    pub Request<Message>,
);

#[derive(Clone, Doom)]
pub(in crate::unicast) enum CasterError {
    #[doom(description("Failed to connect"))]
    ConnectFailed,
    #[doom(description("Interrupted while connecting"))]
    ConnectInterrupted,
    #[doom(description("`Caster` congested"))]
    CasterCongested,
    #[doom(description("`Caster` terminated"))]
    CasterTerminated {
        in_result: Result<(), Stack>,
        out_result: Result<(), Stack>,
    },
}

#[derive(Clone, Doom)]
enum DriveInError {
    #[doom(description("`drive_in` interrupted"))]
    DriveInInterrupted,
    #[doom(description("Connection error"))]
    ConnectionError,
}

#[derive(Clone, Doom)]
enum DriveOutError {
    #[doom(description("`drive_out` interrupted"))]
    DriveOutInterrupted,
    #[doom(description("Connection error"))]
    ConnectionError,
}

impl<Message> Caster<Message>
where
    Message: UnicastMessage,
{
    pub fn new(connector: Arc<dyn Connector>, remote: PublicKey) -> Self {
        let (message_inlet, message_outlet) = mpsc::channel(1024); // TODO: Add settings

        let state = Arc::new(Mutex::new(State::Running(message_inlet)));
        let fuse = Fuse::new();

        {
            let state = state.clone();

            let relay = fuse.relay();

            tokio::spawn(async move {
                Caster::run(connector, remote, message_outlet, state, relay)
                    .await;
            });
        }

        Caster { state, _fuse: fuse }
    }

    pub fn push(
        &self,
        request: Request<Message>,
    ) -> Result<AcknowledgementOutlet, CasterTerminated<Message>> {
        match &*self.state.lock().unwrap() {
            State::Running(message_inlet) => {
                let (acknowledgement_inlet, acknowledgement_outlet) =
                    oneshot::channel();

                match message_inlet.try_send((request, acknowledgement_inlet)) {
                    Ok(()) => {}
                    Err(error) => {
                        let acknowledgement_inlet = match error {
                            TrySendError::Full((_, acknowledgement_inlet)) => {
                                acknowledgement_inlet
                            }
                            TrySendError::Closed(_) => {
                                unreachable!()
                            }
                        };

                        let _ = acknowledgement_inlet
                            .send(CasterError::CasterCongested.fail());
                    }
                }

                Ok(acknowledgement_outlet)
            }
            State::Terminated => Err(CasterTerminated(request)),
        }
    }

    async fn run(
        connector: Arc<dyn Connector>,
        remote: PublicKey,
        mut message_outlet: RequestOutlet<Message>,
        state: Arc<Mutex<State<Message>>>,
        mut relay: Relay,
    ) {
        let database = Arc::new(Mutex::new(Database {
            acknowledgement_inlets: HashMap::new(),
        }));

        let result: Result<(), Top<CasterError>> = async {
            let connection = relay
                .map(connector.connect(remote))
                .await
                .pot(CasterError::ConnectInterrupted, here!())?
                .pot(CasterError::ConnectFailed, here!())?;

            let (sender, receiver) = connection.split();

            let database_in = database.clone();
            let database_out = database.clone();

            let mikado_in = Mikado::new();
            let mikado_out = mikado_in.try_clone().unwrap();

            mikado_in.depend(relay);

            let in_future =
                Caster::<Message>::drive_in(database_in, receiver, mikado_in);

            let out_future = Caster::<Message>::drive_out(
                database_out,
                sender,
                &mut message_outlet,
                mikado_out,
            );

            let (in_result, out_result) = tokio::join!(in_future, out_future);

            CasterError::CasterTerminated {
                in_result: in_result.map_err(Into::into),
                out_result: out_result.map_err(Into::into),
            }
            .fail()
        }
        .await;

        let error = result.unwrap_err();
        *state.lock().unwrap() = State::Terminated;

        Caster::<Message>::clean(database, message_outlet, error).await;
    }

    async fn drive_in(
        database: Arc<Mutex<Database>>,
        mut receiver: SecureReceiver,
        mut mikado: Mikado,
    ) -> Result<(), Top<DriveInError>> {
        loop {
            let response = mikado
                .map(receiver.receive())
                .await
                .pot(DriveInError::DriveInInterrupted, here!())?
                .pot(DriveInError::ConnectionError, here!())?;

            match response {
                Response::Acknowledgement(sequence, acknowledgement) => {
                    if let Some(acknowledgement_inlet) = database
                        .lock()
                        .unwrap()
                        .acknowledgement_inlets
                        .remove(&sequence)
                    {
                        let _ = acknowledgement_inlet.send(Ok(acknowledgement));
                    }
                }
            }
        }
    }

    async fn drive_out(
        database: Arc<Mutex<Database>>,
        mut sender: SecureSender,
        message_outlet: &mut RequestOutlet<Message>,
        mut mikado: Mikado,
    ) -> Result<(), Top<DriveOutError>> {
        for sequence in 0..u32::MAX {
            if let Some((message, acknowledgement_inlet)) = mikado
                .map(message_outlet.recv())
                .await
                .pot(DriveOutError::DriveOutInterrupted, here!())?
            {
                mikado
                    .map(sender.send(&Request::Message(message)))
                    .await
                    .pot(DriveOutError::DriveOutInterrupted, here!())?
                    .pot(DriveOutError::ConnectionError, here!())?;

                database
                    .lock()
                    .unwrap()
                    .acknowledgement_inlets
                    .insert(sequence, acknowledgement_inlet);
            }
        }

        Ok(())
    }

    async fn clean(
        database: Arc<Mutex<Database>>,
        mut message_outlet: RequestOutlet<Message>,
        error: Top<CasterError>,
    ) {
        let mut database = database.lock().unwrap();

        for (_, acknowledgement_inlet) in
            database.acknowledgement_inlets.drain()
        {
            let _ = acknowledgement_inlet.send(Err(error.clone()));
        }

        while let Ok((_, acknowledgement_inlet)) = message_outlet.try_recv() {
            let _ = acknowledgement_inlet.send(Err(error.clone()));
        }
    }
}
