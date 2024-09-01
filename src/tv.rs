use rouille::Response;
use std::fmt;

#[derive(Debug)]
pub enum TVError {
    Other(Box<dyn std::error::Error + Sync + Send>),
}

impl std::error::Error for TVError {}
impl fmt::Display for TVError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Other(err) => write!(f, "Application-specific error: {}", err),
        }
    }
}

impl From<std::io::Error> for TVError {
    fn from(err: std::io::Error) -> Self {
        Self::Other(Box::new(err))
    }
}

impl From<TVError> for Response {
    fn from(e: TVError) -> Self {
        Response::text(e.to_string()).with_status_code(500)
    }
}

pub enum Input {
    HDMI1,
    HDMI2,
    HDMI3,
    HDMI4,
}

pub trait TVConnection {
    fn on_off(&mut self, on: bool) -> Result<(), TVError>;
    fn volume_change(&mut self, relative_steps: i32) -> Result<(), TVError>;
    fn mute(&mut self, mute: bool) -> Result<(), TVError>;
    fn set_input(&mut self, input: Input) -> Result<(), TVError>;
}
