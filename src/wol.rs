use nix::ifaddrs::getifaddrs;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

const SYNCHRONIZATION_SCHEME: [u8; 6] = [0xff; 6];

fn get_broadcast_addr() -> io::Result<IpAddr> {
    let addrs = getifaddrs()?;
    for ifaddr in addrs {
        if !ifaddr
            .flags
            .contains(nix::net::if_::InterfaceFlags::IFF_LOOPBACK)
        {
            if let Some(broadcast) = ifaddr.broadcast {
                if let Some(broadcast_in) = broadcast.as_sockaddr_in() {
                    return Ok(Ipv4Addr::from(broadcast_in.ip()).into());
                }
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "No suitable broadcast address found",
    ))
}

pub fn create_magic_packet(mac_address: [u8; 6]) -> Vec<u8> {
    let mut data: Vec<u8> = SYNCHRONIZATION_SCHEME.to_vec();
    for _ in 0..16 {
        data.extend(&mac_address);
    }
    data
}

pub fn wake(mac_address: [u8; 6]) -> std::io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let data = create_magic_packet(mac_address);
    socket.set_broadcast(true)?;
    let broadcast_addr = get_broadcast_addr()?;
    let addr = SocketAddr::new(broadcast_addr, 7);
    socket.send_to(&data, addr)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_magic_packet() {
        let mac: [u8; 6] = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let packet = create_magic_packet(mac);

        // Check length: 6 bytes header + 16 * 6 bytes MAC = 102 bytes
        assert_eq!(packet.len(), 102);

        // Check header
        assert_eq!(&packet[0..6], &[0xff; 6]);

        // Check MAC repetitions
        for i in 0..16 {
            let offset = 6 + i * 6;
            assert_eq!(&packet[offset..offset + 6], &mac);
        }
    }
}
