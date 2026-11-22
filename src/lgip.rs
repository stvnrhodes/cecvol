use crate::tv;
use crate::tv::TVError;
use crate::wol;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit};
use block_padding::{NoPadding, Pkcs7};
use log::info;
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2;
use std::convert::TryInto;
use std::io;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

// Protocol logic is ported from https://github.com/WesSouza/lgtv-ip-control

const LG_CONTROL_PORT: u16 = 9761;
const ENCRYPTION_KEY_SALT: &[u8] = &[
    99, 97, 184, 14, 155, 220, 166, 99, 141, 7, 32, 242, 204, 86, 143, 185,
];
const ENCRYPTION_IV_LENGTH: usize = 16;
const ENCRYPTION_KEY_LENGTH: usize = 16;
const ENCRYPTION_KEY_ITERATIONS: u32 = 1 << 14;
const RESPONSE_TERMINATOR: u8 = b'\n';
// encryptionKeyDigest: "sha256",

#[derive(Debug)]
pub enum DecryptionError {
    PaddingError(block_padding::UnpadError),
    SliceError(std::array::TryFromSliceError),
    Utf8Error(std::str::Utf8Error),
    InvalidLength,
}

impl std::fmt::Display for DecryptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DecryptionError::PaddingError(_) => write!(f, "Padding error"),
            DecryptionError::SliceError(e) => write!(f, "Slice error: {}", e),
            DecryptionError::Utf8Error(e) => write!(f, "UTF-8 error: {}", e),
            DecryptionError::InvalidLength => write!(f, "Invalid length"),
        }
    }
}

impl std::error::Error for DecryptionError {}

impl From<block_padding::UnpadError> for DecryptionError {
    fn from(err: block_padding::UnpadError) -> Self {
        DecryptionError::PaddingError(err)
    }
}

impl From<std::array::TryFromSliceError> for DecryptionError {
    fn from(err: std::array::TryFromSliceError) -> Self {
        DecryptionError::SliceError(err)
    }
}

impl From<std::str::Utf8Error> for DecryptionError {
    fn from(err: std::str::Utf8Error) -> Self {
        DecryptionError::Utf8Error(err)
    }
}

pub struct LGTV {
    addr: String,
    mac_address: [u8; 6],
    derived_key: [u8; ENCRYPTION_KEY_LENGTH],
}

fn derived_key(keycode: &str) -> [u8; ENCRYPTION_KEY_LENGTH] {
    let mut buf = [0; ENCRYPTION_KEY_LENGTH];
    pbkdf2_hmac::<sha2::Sha256>(
        keycode.as_bytes(),
        ENCRYPTION_KEY_SALT,
        ENCRYPTION_KEY_ITERATIONS,
        &mut buf,
    );
    buf
}

impl LGTV {
    pub fn new(addr: String, mac_address: [u8; 6], keycode: &str) -> Self {
        Self {
            addr,
            mac_address,
            derived_key: derived_key(keycode),
        }
    }
    fn encrypt(&self, cmd: &str) -> Vec<u8> {
        let mut iv = [0; ENCRYPTION_IV_LENGTH];
        rand::thread_rng().fill_bytes(&mut iv);
        self.encrypt_with_iv(cmd, &iv)
    }
    fn encrypt_with_iv(&self, cmd: &str, iv: &[u8]) -> Vec<u8> {
        let iv_encryptor = ecb::Encryptor::<aes::Aes128>::new(&self.derived_key.into());
        let encryptor = cbc::Encryptor::<aes::Aes128>::new(&self.derived_key.into(), iv.into());

        let mut encoded = iv_encryptor.encrypt_padded_vec_mut::<NoPadding>(iv);
        encoded.extend(encryptor.encrypt_padded_vec_mut::<Pkcs7>(cmd.as_bytes()));
        encoded
    }
    fn decrypt(&self, cipher: &[u8]) -> Result<String, DecryptionError> {
        if cipher.len() < ENCRYPTION_KEY_LENGTH {
            return Err(DecryptionError::InvalidLength);
        }
        let iv_decryptor = ecb::Decryptor::<aes::Aes128>::new(&self.derived_key.into());
        let iv_vec = iv_decryptor
            .decrypt_padded_vec_mut::<NoPadding>(cipher[..ENCRYPTION_KEY_LENGTH].into())?;
        let iv: [u8; ENCRYPTION_IV_LENGTH] = iv_vec
            .try_into()
            .map_err(|_| DecryptionError::InvalidLength)?;

        let decryptor = cbc::Decryptor::<aes::Aes128>::new(&self.derived_key.into(), &iv.into());
        let decrypted = decryptor
            .decrypt_padded_vec_mut::<NoPadding>(cipher[ENCRYPTION_KEY_LENGTH..].into())?;
        let end = decrypted
            .iter()
            .position(|&x| x == RESPONSE_TERMINATOR)
            .unwrap_or(0);
        let plaintext = std::str::from_utf8(&decrypted[..end])?;
        Ok(plaintext.to_string())
    }
    pub fn send_command(&self, cmd: &str) -> io::Result<String> {
        let addr = (self.addr.as_str(), LG_CONTROL_PORT)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::AddrNotAvailable,
                    format!("could not resolve TV address: {}", self.addr),
                )
            })?;
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(200))?;
        stream.set_read_timeout(Some(Duration::from_millis(500)))?;
        stream.set_write_timeout(Some(Duration::from_millis(500)))?;
        let payload = self.encrypt(cmd);
        stream.write_all(&payload)?;
        let mut resp = [0; 512];
        let len = stream.read(&mut resp)?;
        let decrypted = self
            .decrypt(&resp[..len])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        info!("{}", decrypted);
        Ok(decrypted)
    }
}

fn parse_volume(resp: &str) -> io::Result<i32> {
    let vol_str = resp.trim().strip_prefix("VOL:").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected volume response: {resp}"),
        )
    })?;
    vol_str
        .parse::<i32>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

impl tv::TVConnection for LGTV {
    fn on_off(&mut self, on: bool) -> Result<(), TVError> {
        if on {
            wol::wake(self.mac_address)?;
        } else {
            self.send_command("POWER off\r")?;
        }
        Ok(())
    }
    fn volume_change(&mut self, relative_steps: i32) -> Result<(), TVError> {
        if relative_steps == 0 {
            return Ok(());
        }
        // For multi-step changes (e.g. Google Home voice commands), read current volume and
        // set directly in 2 round-trips instead of opening up to 25 sequential TCP connections.
        if relative_steps.abs() > 1 {
            if let Ok(Some(current)) = self.get_volume() {
                return self.set_volume(current + relative_steps);
            }
        }
        if relative_steps < 0 {
            for _ in 0..-relative_steps {
                self.send_command("KEY_ACTION volumedown\r")?;
            }
        } else {
            for _ in 0..relative_steps {
                self.send_command("KEY_ACTION volumeup\r")?;
            }
        }
        Ok(())
    }
    fn set_volume(&mut self, level: i32) -> Result<(), TVError> {
        let clamped = level.clamp(0, 100);
        let cmd = format!("VOLUME_CONTROL {clamped}\r");
        self.send_command(&cmd)?;
        Ok(())
    }
    fn get_volume(&mut self) -> Result<Option<i32>, TVError> {
        let resp = self.send_command("CURRENT_VOL\r")?;
        Ok(Some(parse_volume(&resp)?))
    }
    fn mute(&mut self, mute: bool) -> Result<(), TVError> {
        let m = if mute { "on" } else { "off" };
        let cmd = format!("VOLUME_MUTE {m}\r");
        self.send_command(&cmd)?;
        Ok(())
    }
    fn set_input(&mut self, input: tv::Input) -> Result<(), TVError> {
        let input = match input {
            tv::Input::HDMI1 => "hdmi1",
            tv::Input::HDMI2 => "hdmi2",
            tv::Input::HDMI3 => "hdmi3",
            tv::Input::HDMI4 => "hdmi4",
        };
        let cmd = format!("INPUT_SELECT {input}\r");
        self.send_command(&cmd)?;
        Ok(())
    }
    fn play_pause(&mut self) -> Result<(), TVError> {
        self.send_command("KEY_ACTION play\r")?;
        Ok(())
    }
    fn send_key(&mut self, key: tv::Key) -> Result<(), TVError> {
        let action = match key {
            tv::Key::Up => "arrowup",
            tv::Key::Down => "arrowdown",
            tv::Key::Left => "arrowleft",
            tv::Key::Right => "arrowright",
            tv::Key::Ok => "ok",
            tv::Key::Back => "returnback",
            tv::Key::Home => "myapp",
        };
        self.send_command(&format!("KEY_ACTION {action}\r"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::lgip::*;

    #[test]
    fn test_derived_key() {
        let addr = "127.0.0.1".to_string();
        let tv = LGTV::new(addr, [0; 6], "0J8FOLOW");
        assert_eq!(
            &tv.derived_key,
            &[
                0xa4, 0x73, 0x89, 0xda, 0x2c, 0x83, 0xd0, 0x9a, 0x9c, 0x8d, 0x05, 0xa8, 0x28, 0xe7,
                0xf6, 0x5c
            ]
        );
    }

    #[test]
    fn test_encrypt() {
        let addr = "127.0.0.1".to_string();
        let tv = LGTV::new(addr, [0; 6], "0J8FOLOW");
        let iv: &[u8] = &[
            0x82, 0xf2, 0x9e, 0x11, 0xc1, 0x00, 0xd5, 0x3f, 0x7b, 0x14, 0xfe, 0x18, 0x29, 0xc3,
            0x42, 0xf9,
        ];
        let encrypted = tv.encrypt_with_iv("VOLUME_CONTROL 11\r", iv);
        assert_eq!(
            &encrypted,
            &[
                0x54, 0x09, 0xf1, 0x0b, 0xd3, 0x9b, 0x41, 0x67, 0xc3, 0x98, 0x1f, 0xb1, 0x71, 0x2e,
                0x1c, 0xa8, // ivEnc
                0x52, 0x3d, 0x15, 0x71, 0xe8, 0x7e, 0xfb, 0xc4, 0x44, 0xba, 0xcc, 0xc0, 0xb6, 0xca,
                0xb0, 0xeb, 0xdc, 0x80, 0x53, 0x41, 0xa1, 0x18, 0xa4, 0xb3, 0x8d, 0x7a, 0x4e, 0xf4,
                0x94, 0x17, 0xb7, 0x0d, // dataEnc
            ]
        );
    }

    #[test]
    fn test_decrypt() {
        let addr = "127.0.0.1".to_string();
        let tv = LGTV::new(addr, [0; 6], "0J8FOLOW");
        let encrypted: &[u8] = &[
            0xc0, 0xbb, 0x05, 0x47, 0x98, 0x6f, 0x20, 0x5d, 0xeb, 0x67, 0x35, 0xad, 0x07, 0x45,
            0x89, 0xd5, 0xfa, 0xde, 0x82, 0xcd, 0x0d, 0x11, 0x50, 0xb0, 0xda, 0x7f, 0xc0, 0x2e,
            0xb7, 0x24, 0x24, 0x26,
        ];
        let decrypted = tv.decrypt(encrypted).unwrap();
        assert_eq!(&decrypted, "OK");
    }

    #[test]
    fn test_parse_volume() {
        assert_eq!(parse_volume("VOL:19").unwrap(), 19);
        assert_eq!(parse_volume("VOL:0\r\n").unwrap(), 0);
        assert!(parse_volume("OK").is_err());
    }
}
