use crate::net::datagram_dispatcher::Message;

use std::collections::VecDeque;

pub(in crate::net::datagram_dispatcher) struct MessageTable {
    messages: VecDeque<Option<Message>>,
    offset: usize,
}

impl MessageTable {
    pub fn new() -> MessageTable {
        MessageTable {
            messages: VecDeque::new(),
            offset: 0,
        }
    }

    pub fn push(&mut self, message: Message) {
        self.messages.push_back(Some(message));
    }

    pub fn get(&self, index: usize) -> Option<&Message> {
        if index >= self.offset {
            self.messages
                .get(index - self.offset)
                .map(Option::as_ref)
                .flatten()
        } else {
            None
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<Message> {
        if index >= self.offset {
            let message = self
                .messages
                .get_mut(index - self.offset)
                .map(Option::take)
                .flatten();

            while let Some(None) = self.messages.front() {
                self.messages.pop_front();
                self.offset += 1;
            }

            message
        } else {
            None
        }
    }
}
