use std::{
    net::{TcpStream, UdpSocket},
    time::Duration,
};

pub fn scan_tcp(host: String, number: u16) -> Option<bool> {
    let address = format!("{}:{}", host, number);
    match TcpStream::connect(address) {
        Ok(_) => Some(true),
        Err(e) => match e.kind() {
            std::io::ErrorKind::ConnectionRefused => Some(false),
            _ => None,
        },
    }
}
pub fn scan_udp(host: String, number: u16, timeout: Duration) -> Option<bool> {
    let address = format!("{}:{}", host, number);
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    if socket.send_to(&[], address).is_err() {
        return None;
    }
    let mut buffer = [];
    socket.set_read_timeout(Some(timeout)).unwrap();
    match socket.recv(&mut buffer) {
        Ok(_) => Some(true),
        Err(_) => Some(false),
    }
}
