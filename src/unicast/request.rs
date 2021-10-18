use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request<Message> {
    Message(Message),
    KeepAlive,
}
