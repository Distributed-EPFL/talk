use crate::{
    crypto::Identity,
    net::{Connector, SecureReceiver, SecureSender},
    sync::fuse::{Fuse, Relay, Tether},
    unicast::{
        Acknowledgement, CasterSettings, Message as UnicastMessage, Request,
        Response,
    },
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
        result_in: Result<(), Stack>,
        result_out: Result<(), Stack>,
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
    pub fn new(
        connector: Arc<dyn Connector>,
        remote: Identity,
        settings: CasterSettings,
    ) -> Self {
        let (request_inlet, request_outlet) =
            mpsc::channel(settings.request_channel_capacity);

        let state = Arc::new(Mutex::new(State::Running(request_inlet)));
        let fuse = Fuse::new();

        {
            let state = state.clone();

            let relay = fuse.relay();

            tokio::spawn(async move {
                Caster::run(connector, remote, request_outlet, state, relay)
                    .await;
            });
        }

        Caster { state, _fuse: fuse }
    }

    pub fn post(
        &self,
        request: Request<Message>,
    ) -> Result<AcknowledgementOutlet, CasterTerminated<Message>> {
        match &*self.state.lock().unwrap() {
            State::Running(request_inlet) => {
                let (acknowledgement_inlet, acknowledgement_outlet) =
                    oneshot::channel();

                match request_inlet.try_send((request, acknowledgement_inlet)) {
                    Ok(()) => {}
                    Err(error) => {
                        let acknowledgement_inlet = match error {
                            TrySendError::Full((_, acknowledgement_inlet)) => {
                                acknowledgement_inlet
                            }
                            TrySendError::Closed(_) => {
                                // This is unreachable because `acknowledgement_outlet` is owned by `self`:
                                // if `acknowledgement_outlet` was dropped, it would be impossible to call
                                // `self.push` in the first place
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
        remote: Identity,
        mut request_outlet: RequestOutlet<Message>,
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

            let tether_in = Tether::new();
            let tether_out = tether_in.try_clone().unwrap();

            tether_in.depend(relay);

            let future_in =
                Caster::<Message>::drive_in(database_in, receiver, tether_in);

            let future_out = Caster::<Message>::drive_out(
                database_out,
                sender,
                &mut request_outlet,
                tether_out,
            );

            let (result_in, result_out) = tokio::join!(future_in, future_out);

            CasterError::CasterTerminated {
                result_in: result_in.map_err(Into::into),
                result_out: result_out.map_err(Into::into),
            }
            .fail()
        }
        .await;

        let error = result.unwrap_err();
        *state.lock().unwrap() = State::Terminated;

        Caster::<Message>::clean(database, request_outlet, error).await;
    }

    async fn drive_in(
        database: Arc<Mutex<Database>>,
        mut receiver: SecureReceiver,
        mut tether: Tether,
    ) -> Result<(), Top<DriveInError>> {
        loop {
            let response = tether
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
        request_outlet: &mut RequestOutlet<Message>,
        mut tether: Tether,
    ) -> Result<(), Top<DriveOutError>> {
        for sequence in 0..u32::MAX {
            if let Some((request, acknowledgement_inlet)) = tether
                .map(request_outlet.recv())
                .await
                .pot(DriveOutError::DriveOutInterrupted, here!())?
            {
                database
                    .lock()
                    .unwrap()
                    .acknowledgement_inlets
                    .insert(sequence, acknowledgement_inlet);

                tether
                    .map(sender.send(&request))
                    .await
                    .pot(DriveOutError::DriveOutInterrupted, here!())?
                    .pot(DriveOutError::ConnectionError, here!())?;
            }
        }

        Ok(())
    }

    async fn clean(
        database: Arc<Mutex<Database>>,
        mut request_outlet: RequestOutlet<Message>,
        error: Top<CasterError>,
    ) {
        let mut database = database.lock().unwrap();

        for (_, acknowledgement_inlet) in
            database.acknowledgement_inlets.drain()
        {
            let _ = acknowledgement_inlet.send(Err(error.clone()));
        }

        while let Ok((_, acknowledgement_inlet)) = request_outlet.try_recv() {
            let _ = acknowledgement_inlet.send(Err(error.clone()));
        }
    }
}
