use cec_rs::{CecConnection, CecConnectionResultError};
use std::fmt;

#[derive(Debug)]
pub struct CECError {
    err: CecConnectionResultError,
}
impl actix_http::ResponseError for CECError {}
impl fmt::Display for CECError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.err)
    }
}
impl From<CecConnectionResultError> for CECError {
    fn from(err: CecConnectionResultError) -> CECError {
        CECError { err: err }
    }
}

pub struct CEC {
    conn: CecConnection,
}

impl CEC {
    pub fn new(conn: CecConnection) -> Self {
        CEC { conn }
    }

    pub fn volume_change(&self, relative_steps: i32) -> Result<(), CECError> {
        if relative_steps > 0 {
            for _ in 0..relative_steps {
                self.conn.volume_up(true)?;
            }
        } else if relative_steps < 0 {
            for _ in relative_steps..0 {
                self.conn.volume_down(true)?;
            }
        }
        Ok(())
    }
}
unsafe impl Send for CEC {}
