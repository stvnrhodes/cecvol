use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
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

impl IntoResponse for TVError {
    fn into_response(self) -> Response {
        StatusCode::IM_A_TEAPOT.into_response()
    }
}

pub enum Input {
    HDMI1,
    HDMI2,
    HDMI3,
    HDMI4,
}

pub trait TVConnection {
    fn on_off(&self, on: bool) -> Result<(), TVError>;
    fn volume_change(&self, relative_steps: i32) -> Result<(), TVError>;
    fn mute(&self, mute: bool) -> Result<(), TVError>;
    fn set_input(&self, input: Input) -> Result<(), TVError>;
}
