use doomstack::{here, Doom, ResultExt, Top};
use parking_lot::{Mutex, MutexGuard};
use std::ops::{Deref, DerefMut};

pub struct Voidable<Inner>(Mutex<Option<Inner>>);

#[derive(Doom)]
pub enum VoidableError {
    #[doom(description("`Voidable` voided"))]
    Void,
}

pub struct VoidableGuard<'v, Inner>(MutexGuard<'v, Option<Inner>>);

impl<Inner> Voidable<Inner> {
    pub fn new(inner: Inner) -> Self {
        Voidable(Mutex::new(Some(inner)))
    }

    pub fn lock<'v>(&'v self) -> Result<VoidableGuard<'v, Inner>, Top<VoidableError>> {
        let guard = self.0.lock();
        if guard.is_some() {
            Ok(VoidableGuard(guard))
        } else {
            VoidableError::Void.fail().spot(here!())
        }
    }

    pub fn void(&self) -> Inner {
        self.0
            .lock()
            .take()
            .expect("called `Voidable::void` on an already voided `Voidable`")
    }
}

impl<'v, Inner> Deref for VoidableGuard<'v, Inner> {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl<'v, Inner> DerefMut for VoidableGuard<'v, Inner> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn void() {
        let voidable = Voidable::new(33u32);

        {
            let mut voidable = voidable.lock().unwrap();
            assert_eq!(*voidable, 33);

            *voidable = 99;
        }

        {
            let mut voidable = voidable.lock().unwrap();
            assert_eq!(*voidable, 99);

            *voidable = 11;
        }

        assert_eq!(voidable.void(), 11);

        assert!(voidable.lock().is_err());
    }
}
