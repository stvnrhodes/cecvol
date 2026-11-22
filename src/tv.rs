use std::fmt;

#[derive(Debug)]
pub enum TVError {
    Unsupported(String),
    Other(Box<dyn std::error::Error + Sync + Send>),
}

impl std::error::Error for TVError {}
impl fmt::Display for TVError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Unsupported(msg) => write!(f, "Unsupported operation: {}", msg),
            Self::Other(err) => write!(f, "Application-specific error: {}", err),
        }
    }
}

impl From<std::io::Error> for TVError {
    fn from(err: std::io::Error) -> Self {
        Self::Other(Box::new(err))
    }
}

pub enum Input {
    HDMI1,
    HDMI2,
    HDMI3,
    HDMI4,
}

pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Ok,
    Back,
    Home,
}

pub trait TVConnection {
    fn on_off(&mut self, on: bool) -> Result<(), TVError>;
    fn volume_change(&mut self, relative_steps: i32) -> Result<(), TVError>;
    fn set_volume(&mut self, level: i32) -> Result<(), TVError>;
    fn get_volume(&mut self) -> Result<Option<i32>, TVError>;
    fn mute(&mut self, mute: bool) -> Result<(), TVError>;
    fn set_input(&mut self, input: Input) -> Result<(), TVError>;
    fn play_pause(&mut self) -> Result<(), TVError>;
    fn send_key(&mut self, key: Key) -> Result<(), TVError>;
}
