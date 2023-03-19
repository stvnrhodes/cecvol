use crate::tv;
use crate::tv::TVError;
use crate::wol;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit};
use block_padding::{NoPadding, Pkcs7};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2;
use std::convert::TryInto;
use std::{
    io,
    net::{IpAddr, UdpSocket},
    time::Duration,
};

// Protocol logic is ported from https://github.com/WesSouza/lgtv-ip-control

const LG_CONTROL_PORT: u16 = 9761;
const ENCRYPTION_KEY_SALT: &[u8] = &[
    99, 97, 184, 14, 155, 220, 166, 99, 141, 7, 32, 242, 204, 86, 143, 185,
];
const ENCRYPTION_IV_LENGTH: usize = 16;
const ENCRYPTION_KEY_LENGTH: usize = 16;
const ENCRYPTION_KEY_ITERATIONS: u32 = 1 << 14;
const MESSAGE_BLOCK_SIZE: usize = 16;
const MESSAGE_TERMINATOR: char = '\r';
const RESPONSE_TERMINATOR: u8 = b'\n';
// encryptionKeyDigest: "sha256",

pub struct LGTV {
    ip_addr: IpAddr,
    mac_address: [u8; 6],
    derived_key: [u8; ENCRYPTION_KEY_LENGTH],
}

// info!("faking command {:?}", cmd);

fn derived_key(keycode: &str) -> [u8; ENCRYPTION_KEY_LENGTH] {
    // return pbkdf2Sync(
    //     keycode,
    //     Buffer.from(settings.encryptionKeySalt),
    //     settings.encryptionKeyIterations,
    //     settings.encryptionKeyLength,
    //     settings.encryptionKeyDigest,
    //   );
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
    pub fn new(ip_addr: IpAddr, mac_address: [u8; 6], keycode: &str) -> Self {
        Self {
            ip_addr,
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
    fn decrypt(&self, cipher: &[u8]) -> Result<String, std::str::Utf8Error> {
        // TODO: Don't unwrap
        let iv_decryptor = ecb::Decryptor::<aes::Aes128>::new(&self.derived_key.into());
        let iv_vec = iv_decryptor
            .decrypt_padded_vec_mut::<NoPadding>(cipher[..ENCRYPTION_KEY_LENGTH].into())
            .unwrap();
        let iv: [u8; ENCRYPTION_IV_LENGTH] = iv_vec.try_into().unwrap();

        let decryptor = cbc::Decryptor::<aes::Aes128>::new(&self.derived_key.into(), &iv.into());
        let decrypted = decryptor
            .decrypt_padded_vec_mut::<NoPadding>(cipher[ENCRYPTION_KEY_LENGTH..].into())
            .unwrap();
        let end = decrypted
            .iter()
            .position(|&x| x == RESPONSE_TERMINATOR)
            .unwrap_or(0);
        let plaintext = std::str::from_utf8(&decrypted[..end])?;
        Ok(plaintext.to_string())
    }
    pub fn send_command(&self, cmd: &str) -> io::Result<String> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.connect((self.ip_addr, LG_CONTROL_PORT))?;
        socket.set_read_timeout(Some(Duration::new(10, 0)))?;
        let payload = self.encrypt(cmd);
        socket.send(&payload)?;
        let mut resp = [0; 512];
        let len = socket.recv(&mut resp)?;
        let decrypted = self.decrypt(&resp[..len]);
        // TODO: Convert error
        Ok(decrypted.unwrap())
    }
}

impl tv::TVConnection for LGTV {
    fn power_on(&self) -> Result<(), TVError> {
        wol::wake(self.mac_address)?;
        Ok(())
    }
    fn power_off(&self) -> Result<(), TVError> {
        self.send_command("POWER off\r")?;
        Ok(())
    }
    fn vol_up(&self) -> Result<(), TVError> {
        self.send_command("KEY_ACTION volumeup\r")?;
        Ok(())
    }
    fn vol_down(&self) -> Result<(), TVError> {
        self.send_command("KEY_ACTION volumedown\r")?;
        Ok(())
    }
    fn mute(&self, mute: bool) -> Result<(), TVError> {
        let m = if mute { "on" } else { "off" };
        let cmd = format!("VOLUME_MUTE {m}\r");
        self.send_command(&cmd)?;
        Ok(())
    }
    fn input(&self, input: tv::Input) -> Result<(), TVError> {
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
}

// message INPUT_SELECT hdmi1
// iv <Buffer 2d df 1f 39 e3 f6 ee c0 2d 1b c3 e4 d8 cb 6d 0a>
// preparedMessage INPUT_SELECT hdmi1
// derivedKey <Buffer a4 73 89 da 2c 83 d0 9a 9c 8d 05 a8 28 e7 f6 5c>
// ivEnc <Buffer ff 77 d9 42 7b d3 03 b1 57 cd b3 3a 77 41 2a 3d>
// dataEnc <Buffer 41 f4 ca 01 09 36 d3 72 a0 24 2b cb 48 a1 89 8d 8a 9a 23 89 14 ab 42 39 25 8f c4 a6 4e da b8 a7>
// cipher <Buffer af 5b e7 25 f6 26 7b 8a 8a 1a 31 11 65 a5 ec 53 44 cd bd c0 34 ab 3b 19 f3 05 12 d7 d3 34 e3 30>
// ivRecv <Buffer 3e 53 c1 04 7d bf f9 11 ef 14 41 7c 71 0f 70 4e>
// decrypted OK

// iv <Buffer 82 f2 9e 11 c1 00 d5 3f 7b 14 fe 18 29 c3 42 f9>
// preparedMessage VOLUME_CONTROL 11
// derivedKey <Buffer a4 73 89 da 2c 83 d0 9a 9c 8d 05 a8 28 e7 f6 5c>
// ivEnc <Buffer 54 09 f1 0b d3 9b 41 67 c3 98 1f b1 71 2e 1c a8>
// dataEnc <Buffer 52 3d 15 71 e8 7e fb c4 44 ba cc c0 b6 ca b0 eb dc 80 53 41 a1 18 a4 b3 8d 7a 4e f4 94 17 b7 0d>
// cipher <Buffer c0 bb 05 47 98 6f 20 5d eb 67 35 ad 07 45 89 d5 fa de 82 cd 0d 11 50 b0 da 7f c0 2e b7 24 24 26>
// ivRecv <Buffer 86 e6 08 6b 04 cf 4b 26 3b 77 24 6c bd 8b e0 b1>

// iv <Buffer f3 b0 0d 21 c4 5e 98 0a e9 99 dd 2f 90 87 30 4b>
// preparedMessage VOLUME_CONTROL 11
// derivedKey <Buffer a4 73 89 da 2c 83 d0 9a 9c 8d 05 a8 28 e7 f6 5c>
// ivEnc <Buffer 94 3c 62 c1 d4 a5 d0 71 7d 29 2b f6 5e 9d 8a f6>
// dataEnc <Buffer 99 a6 1f 23 54 fa f8 d8 a9 90 ac 24 66 4a e9 47 5d 50 ea 43 8a cb 37 bd 5e 39 a2 bf 06 1d 23 1f>
// cipher <Buffer 69 f2 7c 71 43 c3 59 a5 ce ee 1e 04 ba 22 05 86 51 62 a2 7f 14 2e 35 c5 ad ca 68 50 00 88 5a ae>
// ivRecv <Buffer 64 de d3 4a c7 12 ae 14 dd 71 f6 0e 2d 30 df 9a>
// decrypted OK

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use crate::lgip::*;

    #[test]
    fn test_derived_key() {
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
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
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
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
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let tv = LGTV::new(addr, [0; 6], "0J8FOLOW");
        let encrypted: &[u8] = &[
            0xc0, 0xbb, 0x05, 0x47, 0x98, 0x6f, 0x20, 0x5d, 0xeb, 0x67, 0x35, 0xad, 0x07, 0x45,
            0x89, 0xd5, 0xfa, 0xde, 0x82, 0xcd, 0x0d, 0x11, 0x50, 0xb0, 0xda, 0x7f, 0xc0, 0x2e,
            0xb7, 0x24, 0x24, 0x26,
        ];
        let decrypted = tv.decrypt(encrypted).unwrap();
        assert_eq!(&decrypted, "OK");
    }
}
