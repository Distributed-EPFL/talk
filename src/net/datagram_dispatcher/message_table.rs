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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::net::datagram_dispatcher::MAXIMUM_TRANSMISSION_UNIT;

    #[test]
    fn manual() {
        let mut table = MessageTable::new();

        for index in 0..128 {
            table.push(Message {
                buffer: [0u8; MAXIMUM_TRANSMISSION_UNIT],
                size: index,
            });
        }

        assert_eq!(table.messages.len(), 128);

        for index in 0..128 {
            assert_eq!(table.get(index).unwrap().size, index);
        }

        assert_eq!(table.remove(0).unwrap().size, 0);

        assert_eq!(table.messages.len(), 127);

        assert!(table.get(0).is_none());

        for index in 1..128 {
            assert_eq!(table.get(index).unwrap().size, index);
        }

        assert_eq!(table.remove(2).unwrap().size, 2);
        assert_eq!(table.remove(3).unwrap().size, 3);

        assert_eq!(table.messages.len(), 127);

        for index in [0, 2, 3] {
            assert!(table.get(index).is_none());
        }

        for index in (1..=1).into_iter().chain(4..128) {
            assert_eq!(table.get(index).unwrap().size, index);
        }

        assert_eq!(table.remove(1).unwrap().size, 1);

        assert_eq!(table.messages.len(), 124);

        for index in 0..4 {
            assert!(table.get(index).is_none());
        }

        for index in 4..128 {
            assert_eq!(table.get(index).unwrap().size, index);
        }
    }
}
