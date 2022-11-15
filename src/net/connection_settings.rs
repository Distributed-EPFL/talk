use crate::net::{ReceiverSettings, SenderSettings};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct ConnectionSettings {
    pub send_timeout: Option<Duration>,
    pub receive_timeout: Option<Duration>,
}

const SEND_TIMEOUT_DEFAULT: u64 = 0;
const RECEIVE_TIMEOUT_DEFAULT: u64 = 0;

static SEND_TIMEOUT: AtomicU64 = AtomicU64::new(SEND_TIMEOUT_DEFAULT);
static RECEIVE_TIMEOUT: AtomicU64 = AtomicU64::new(RECEIVE_TIMEOUT_DEFAULT);

impl Default for ConnectionSettings {
    fn default() -> Self {
        let send_timeout = SEND_TIMEOUT.load(Ordering::Relaxed);

        let send_timeout = if send_timeout == 0 {
            None
        } else {
            Some(Duration::from_micros(send_timeout))
        };

        let receive_timeout = RECEIVE_TIMEOUT.load(Ordering::Relaxed);

        let receive_timeout = if receive_timeout == 0 {
            None
        } else {
            Some(Duration::from_micros(receive_timeout))
        };

        ConnectionSettings {
            send_timeout,
            receive_timeout,
        }
    }
}

impl ConnectionSettings {
    pub fn split(self) -> (SenderSettings, ReceiverSettings) {
        (
            SenderSettings {
                send_timeout: self.send_timeout,
            },
            ReceiverSettings {
                receive_timeout: self.receive_timeout,
            },
        )
    }

    pub fn set_default(settings: ConnectionSettings) {
        let send_timeout = if let Some(send_timeout) = settings.send_timeout {
            let send_timeout = send_timeout.as_micros() as u64;

            if send_timeout > 0 {
                send_timeout
            } else {
                panic!("called `ConnectionSettings::set_default` with a null `send_timeout`")
            }
        } else {
            0
        };

        let receive_timeout = if let Some(receive_timeout) = settings.receive_timeout {
            let receive_timeout = receive_timeout.as_micros() as u64;

            if receive_timeout > 0 {
                receive_timeout
            } else {
                panic!("called `ConnectionSettings::set_default` with a null `receive_timeout`")
            }
        } else {
            0
        };

        SEND_TIMEOUT.store(send_timeout, Ordering::Relaxed);
        RECEIVE_TIMEOUT.store(receive_timeout, Ordering::Relaxed);
    }
}
