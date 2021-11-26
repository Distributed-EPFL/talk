use crate::{
    crypto::Identity,
    net::{Connector, SecureReceiver, SecureSender},
    sync::fuse::Fuse,
    unicast::{Acknowledgement, CasterSettings, Message as UnicastMessage, Request, Response},
};

use doomstack::{here, Doom, ResultExt, Top};

use parking_lot::Mutex;

use std::sync::Arc;

use tokio::sync::{
    mpsc,
    mpsc::{error::TrySendError, Receiver as MpscReceiver, Sender as MpscSender},
    oneshot,
    oneshot::{Receiver as OneshotReceiver, Sender as OneshotSender},
};

use std::collections::HashMap;

type RequestInlet<Message> = MpscSender<(Request<Message>, AcknowledgementInlet)>;
type RequestOutlet<Message> = MpscReceiver<(Request<Message>, AcknowledgementInlet)>;

type AcknowledgementInlet = OneshotSender<Result<Acknowledgement, Top<CasterError>>>;
type AcknowledgementOutlet = OneshotReceiver<Result<Acknowledgement, Top<CasterError>>>;

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

pub(in crate::unicast) struct CasterTerminated<Message: UnicastMessage>(pub Request<Message>);

#[derive(Clone, Doom)]
pub(in crate::unicast) enum CasterError {
    #[doom(description("Failed to connect"))]
    ConnectFailed,
    #[doom(description("`Caster` congested"))]
    CasterCongested,
    #[doom(description("`drive_in` failed"))]
    DriveInFailed,
    #[doom(description("`drive_out` failed"))]
    DriveOutFailed,
}

#[derive(Clone, Doom)]
enum DriveInError {
    #[doom(description("Connection error"))]
    ConnectionError,
}

#[derive(Clone, Doom)]
enum DriveOutError {
    #[doom(description("Connection error"))]
    ConnectionError,
}

impl<Message> Caster<Message>
where
    Message: UnicastMessage,
{
    pub fn new(connector: Arc<dyn Connector>, remote: Identity, settings: CasterSettings) -> Self {
        let (request_inlet, request_outlet) = mpsc::channel(settings.request_channel_capacity);

        let state = Arc::new(Mutex::new(State::Running(request_inlet)));
        let fuse = Fuse::new();

        {
            let state = state.clone();

            fuse.spawn(async move {
                Caster::run(connector, remote, request_outlet, state).await;
            });
        }

        Caster { state, _fuse: fuse }
    }

    pub fn post(
        &self,
        request: Request<Message>,
    ) -> Result<AcknowledgementOutlet, CasterTerminated<Message>> {
        match &*self.state.lock() {
            State::Running(request_inlet) => {
                let (acknowledgement_inlet, acknowledgement_outlet) = oneshot::channel();

                match request_inlet.try_send((request, acknowledgement_inlet)) {
                    Ok(()) => {}
                    Err(error) => {
                        let acknowledgement_inlet = match error {
                            TrySendError::Full((_, acknowledgement_inlet)) => acknowledgement_inlet,
                            TrySendError::Closed(_) => {
                                // This is unreachable because `acknowledgement_outlet` is owned by `self`:
                                // if `acknowledgement_outlet` was dropped, it would be impossible to call
                                // `self.push` in the first place
                                unreachable!()
                            }
                        };

                        let _ = acknowledgement_inlet.send(CasterError::CasterCongested.fail());
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
    ) {
        let database = Arc::new(Mutex::new(Database {
            acknowledgement_inlets: HashMap::new(),
        }));

        let result: Result<(), Top<CasterError>> = async {
            let connection = connector
                .connect(remote)
                .await
                .pot(CasterError::ConnectFailed, here!())?;

            let (sender, receiver) = connection.split();

            let result = tokio::try_join!(
                async {
                    Caster::<Message>::drive_in(&*database, receiver)
                        .await
                        .pot(CasterError::DriveInFailed, here!())
                },
                async {
                    Caster::<Message>::drive_out(&*database, sender, &mut request_outlet)
                        .await
                        .pot(CasterError::DriveOutFailed, here!())
                }
            );

            result.map(|_| ())
        }
        .await;

        let error = result.unwrap_err();
        *state.lock() = State::Terminated;

        Caster::<Message>::clean(database, request_outlet, error).await;
    }

    async fn drive_in(
        database: &Mutex<Database>,
        mut receiver: SecureReceiver,
    ) -> Result<(), Top<DriveInError>> {
        loop {
            let response = receiver
                .receive()
                .await
                .pot(DriveInError::ConnectionError, here!())?;

            match response {
                Response::Acknowledgement(sequence, acknowledgement) => {
                    if let Some(acknowledgement_inlet) =
                        database.lock().acknowledgement_inlets.remove(&sequence)
                    {
                        let _ = acknowledgement_inlet.send(Ok(acknowledgement));
                    }
                }
            }
        }
    }

    async fn drive_out(
        database: &Mutex<Database>,
        mut sender: SecureSender,
        request_outlet: &mut RequestOutlet<Message>,
    ) -> Result<(), Top<DriveOutError>> {
        for sequence in 0..u32::MAX {
            if let Some((request, acknowledgement_inlet)) = request_outlet.recv().await {
                database
                    .lock()
                    .acknowledgement_inlets
                    .insert(sequence, acknowledgement_inlet);

                sender
                    .send(&request)
                    .await
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
        let mut database = database.lock();

        for (_, acknowledgement_inlet) in database.acknowledgement_inlets.drain() {
            let _ = acknowledgement_inlet.send(Err(error.clone()));
        }

        while let Ok((_, acknowledgement_inlet)) = request_outlet.try_recv() {
            let _ = acknowledgement_inlet.send(Err(error.clone()));
        }
    }
}
