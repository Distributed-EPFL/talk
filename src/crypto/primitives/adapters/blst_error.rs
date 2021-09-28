use blst::BLST_ERROR;

use crate::crypto::primitives::errors::multi::BlstError;

pub(crate) trait BlstErrorAdapter {
    fn into_result(self) -> Result<(), BlstError>;
}

impl BlstErrorAdapter for BLST_ERROR {
    fn into_result(self) -> Result<(), BlstError> {
        match self {
            BLST_ERROR::BLST_SUCCESS => Ok(()),
            error => Err(error.into()),
        }
    }
}
