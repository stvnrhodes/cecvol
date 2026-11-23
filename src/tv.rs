use log::info;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    HDMI1,
    HDMI2,
    HDMI3,
    HDMI4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Ok,
    Back,
    Home,
    Settings,
}

pub trait TVConnection {
    fn on_off(&mut self, on: bool) -> Result<(), TVError>;
    fn volume_change(&mut self, relative_steps: i32) -> Result<(), TVError>;
    fn set_volume(&mut self, level: i32) -> Result<(), TVError>;
    fn get_volume(&mut self) -> Result<i32, TVError>;
    fn mute(&mut self, mute: bool) -> Result<(), TVError>;
    fn get_mute(&mut self) -> Result<bool, TVError>;
    fn set_input(&mut self, input: Input) -> Result<(), TVError>;
    fn play_pause(&mut self) -> Result<(), TVError>;
    fn send_key(&mut self, key: Key) -> Result<(), TVError>;
}

/// In-memory TV implementation for local testing without real hardware.
pub struct FakeTV {
    on: bool,
    volume: i32,
    muted: bool,
}

impl Default for FakeTV {
    fn default() -> Self {
        Self {
            on: true,
            volume: 20,
            muted: false,
        }
    }
}

impl TVConnection for FakeTV {
    fn on_off(&mut self, on: bool) -> Result<(), TVError> {
        info!("FakeTV: on_off({})", on);
        self.on = on;
        Ok(())
    }
    fn volume_change(&mut self, relative_steps: i32) -> Result<(), TVError> {
        self.volume = (self.volume + relative_steps).clamp(0, 100);
        self.muted = false;
        info!(
            "FakeTV: volume_change({}) -> {}",
            relative_steps, self.volume
        );
        Ok(())
    }
    fn set_volume(&mut self, level: i32) -> Result<(), TVError> {
        self.volume = level.clamp(0, 100);
        self.muted = false;
        info!("FakeTV: set_volume({})", self.volume);
        Ok(())
    }
    fn get_volume(&mut self) -> Result<i32, TVError> {
        if !self.on {
            return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "FakeTV is off").into());
        }
        Ok(self.volume)
    }
    fn mute(&mut self, mute: bool) -> Result<(), TVError> {
        info!("FakeTV: mute({})", mute);
        self.muted = mute;
        Ok(())
    }
    fn get_mute(&mut self) -> Result<bool, TVError> {
        if !self.on {
            return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "FakeTV is off").into());
        }
        Ok(self.muted)
    }
    fn set_input(&mut self, input: Input) -> Result<(), TVError> {
        info!("FakeTV: set_input({:?})", input);
        Ok(())
    }
    fn play_pause(&mut self) -> Result<(), TVError> {
        info!("FakeTV: play_pause");
        Ok(())
    }
    fn send_key(&mut self, key: Key) -> Result<(), TVError> {
        info!("FakeTV: send_key({:?})", key);
        Ok(())
    }
}
