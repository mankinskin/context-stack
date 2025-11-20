use context_trace::*;
use derive_new::new;

use crate::state::result::Response;
use std::fmt::Debug;

//pub(crate) type FoldResult = Result<Response, ErrorState>;

#[derive(Debug, new)]
pub struct ErrorState {
    pub reason: ErrorReason,
    pub found: Option<Box<Response>>,
}
impl From<ErrorReason> for ErrorState {
    fn from(reason: ErrorReason) -> Self {
        Self {
            reason,
            found: None,
        }
    }
}
impl From<IndexWithPath> for ErrorState {
    fn from(value: IndexWithPath) -> Self {
        ErrorReason::SingleIndex(Box::new(value)).into()
    }
}
