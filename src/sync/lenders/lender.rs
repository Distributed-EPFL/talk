use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    borrow::{Borrow, BorrowMut},
    mem,
};

pub struct Lender<Inner> {
    state: State<Inner>,
}

enum State<Inner> {
    Available(Inner),
    Lent,
}

impl<Inner> Lender<Inner> {
    pub fn new(inner: Inner) -> Self {
        Lender {
            state: State::Available(inner),
        }
    }

    pub fn take(&mut self) -> Inner {
        let state = self.try_take();

        match state {
            Some(inner) => inner,
            None => panic!("attempted to `Lender::take` more than once without `Lender::restore`"),
        }
    }

    pub fn try_take(&mut self) -> Option<Inner> {
        let mut state = State::Lent;
        mem::swap(&mut self.state, &mut state);

        match state {
            State::Available(inner) => Some(inner),
            State::Lent => None,
        }
    }

    pub fn restore(&mut self, inner: Inner) {
        if self.try_restore(inner).is_err() {
            panic!("attempted to `Lender::restore` more than once without `Lender::take`");
        }
    }

    pub fn try_restore(&mut self, inner: Inner) -> Result<(), Inner> {
        if let State::Lent = self.state {
            self.state = State::Available(inner);
            Ok(())
        } else {
            Err(inner)
        }
    }
}

impl<Inner> Borrow<Inner> for Lender<Inner> {
    fn borrow(&self) -> &Inner {
        match &self.state {
            State::Available(inner) => &inner,
            State::Lent => panic!("attempted to `borrow` `Lender` without `Lender::restore`"),
        }
    }
}

impl<Inner> BorrowMut<Inner> for Lender<Inner> {
    fn borrow_mut(&mut self) -> &mut Inner {
        match &mut self.state {
            State::Available(inner) => inner,
            State::Lent => panic!("attempted to `borrow_mut `Lender` without `Lender::restore`"),
        }
    }
}

impl<Inner> Serialize for Lender<Inner>
where
    Inner: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.state {
            State::Available(inner) => inner.serialize(serializer),
            State::Lent => panic!("attempted to serialize lent `Lender`"),
        }
    }
}

impl<'de, Inner> Deserialize<'de> for Lender<Inner>
where
    Inner: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inner = Inner::deserialize(deserializer)?;
        Ok(Lender::new(inner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_available() {
        let lender: Lender<u32> = Lender::new(3);
        let serialized = bincode::serialize(&lender).unwrap();
        let mut lender: Lender<u32> = bincode::deserialize(&serialized).unwrap();

        let value = lender.take();
        assert_eq!(value, 3);
        lender.restore(value);
    }

    #[test]
    #[should_panic(expected = "attempted to serialize lent `Lender`")]
    fn serialize_lent() {
        let mut lender: Lender<u32> = Lender::new(3);
        lender.take();
        let _result = bincode::serialize(&lender);
    }
}
