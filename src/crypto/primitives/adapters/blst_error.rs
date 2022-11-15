use blst::BLST_ERROR;
use std::{
    error::Error,
    fmt,
    fmt::{Display, Formatter},
};

#[derive(Debug)]
pub struct BlstError(BLST_ERROR);

pub(crate) trait BlstErrorAdapter {
    fn into_result(self) -> Result<(), BlstError>;
}

impl From<BLST_ERROR> for BlstError {
    fn from(error: BLST_ERROR) -> Self {
        BlstError(error)
    }
}

impl Error for BlstError {}

impl Display for BlstError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let s = match self.0 {
            BLST_ERROR::BLST_POINT_NOT_IN_GROUP => "point not in group",
            BLST_ERROR::BLST_AGGR_TYPE_MISMATCH => "signature type mismatched",
            BLST_ERROR::BLST_PK_IS_INFINITY => "public key is infinity",
            BLST_ERROR::BLST_SUCCESS => "no error",
            BLST_ERROR::BLST_BAD_ENCODING => "bad encoding",
            BLST_ERROR::BLST_POINT_NOT_ON_CURVE => "point not on curve",
            BLST_ERROR::BLST_VERIFY_FAIL => "bad signature",
            BLST_ERROR::BLST_BAD_SCALAR => "bad scalar",
        };

        write!(f, "{}", s)
    }
}

impl BlstErrorAdapter for BLST_ERROR {
    fn into_result(self) -> Result<(), BlstError> {
        match self {
            BLST_ERROR::BLST_SUCCESS => Ok(()),
            error => Err(error.into()),
        }
    }
}
