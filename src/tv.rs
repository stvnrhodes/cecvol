// use axum::response::IntoResponse;
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
// impl IntoResponse for TVError {
//     fn into_response(self) -> Response {
//         StatusCode::IM_A_TEAPOT.into_response()
//     }
// }

pub enum Input {
    HDMI1,
    HDMI2,
    HDMI3,
    HDMI4,
}

pub trait TVConnection: Sync + Send {
    fn power_on(&self) -> Result<(), TVError>;
    fn power_off(&self) -> Result<(), TVError>;
    fn vol_up(&self) -> Result<(), TVError>;
    fn vol_down(&self) -> Result<(), TVError>;
    fn mute(&self, mute: bool) -> Result<(), TVError>;
    fn input(&self, input: Input) -> Result<(), TVError>;
}
