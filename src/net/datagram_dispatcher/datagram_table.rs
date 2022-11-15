use crate::net::datagram_dispatcher::Message;
use std::{collections::VecDeque, net::SocketAddr};

pub(in crate::net::datagram_dispatcher) struct DatagramTable {
    datagrams: VecDeque<Option<(SocketAddr, Message)>>,
    offset: usize,
}

impl DatagramTable {
    pub fn new() -> DatagramTable {
        DatagramTable {
            datagrams: VecDeque::new(),
            offset: 0,
        }
    }

    pub fn push(&mut self, destination: SocketAddr, message: Message) {
        self.datagrams.push_back(Some((destination, message)));
    }

    pub fn get(&self, index: usize) -> Option<&(SocketAddr, Message)> {
        if index >= self.offset {
            self.datagrams
                .get(index - self.offset)
                .and_then(Option::as_ref)
        } else {
            None
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<(SocketAddr, Message)> {
        if index >= self.offset {
            let datagram = self
                .datagrams
                .get_mut(index - self.offset)
                .and_then(Option::take);

            while let Some(None) = self.datagrams.front() {
                self.datagrams.pop_front();
                self.offset += 1;
            }

            datagram
        } else {
            None
        }
    }

    pub fn cursor(&self) -> usize {
        self.offset + self.datagrams.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::datagram_dispatcher::MAXIMUM_TRANSMISSION_UNIT;

    #[test]
    fn manual() {
        let mut table = DatagramTable::new();

        for index in 0..128 {
            table.push(
                "127.0.0.1:1234".parse().unwrap(),
                Message {
                    buffer: [0u8; MAXIMUM_TRANSMISSION_UNIT],
                    size: index,
                },
            );
        }

        assert_eq!(table.datagrams.len(), 128);

        for index in 0..128 {
            assert_eq!(table.get(index).unwrap().1.size, index);
        }

        assert_eq!(table.remove(0).unwrap().1.size, 0);

        assert_eq!(table.datagrams.len(), 127);

        assert!(table.get(0).is_none());

        for index in 1..128 {
            assert_eq!(table.get(index).unwrap().1.size, index);
        }

        assert_eq!(table.remove(2).unwrap().1.size, 2);
        assert_eq!(table.remove(3).unwrap().1.size, 3);

        assert_eq!(table.datagrams.len(), 127);

        for index in [0, 2, 3] {
            assert!(table.get(index).is_none());
        }

        for index in (1..=1).into_iter().chain(4..128) {
            assert_eq!(table.get(index).unwrap().1.size, index);
        }

        assert_eq!(table.remove(1).unwrap().1.size, 1);

        assert_eq!(table.datagrams.len(), 124);

        for index in 0..4 {
            assert!(table.get(index).is_none());
        }

        for index in 4..128 {
            assert_eq!(table.get(index).unwrap().1.size, index);
        }
    }
}
