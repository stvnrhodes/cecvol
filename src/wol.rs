use std::net::UdpSocket;

const SYNCHRONIZATION_SCHEME: [u8; 6] = [0xff; 6];

pub fn wake(mac_address: [u8; 6]) -> std::io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let mut data: Vec<u8> = SYNCHRONIZATION_SCHEME.to_vec();
    for _ in 0..16 {
        data.extend(&mac_address);
    }
    socket.set_broadcast(true)?;
    socket.send_to(&data, "255.255.255.255:7")?;
    Ok(())
}
