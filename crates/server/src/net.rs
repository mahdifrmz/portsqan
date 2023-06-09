use std::{
    net::{SocketAddr, TcpStream, UdpSocket},
    time::Duration,
};

pub fn scan_tcp(host: String, number: u16, timeout: Duration, attemps: usize) -> Option<bool> {
    let mut rsl = None;
    for _ in 0..attemps {
        rsl = try_tcp(host.clone(), number, timeout);
        if rsl == Some(true) {
            return rsl;
        }
    }
    rsl
}
pub fn scan_udp(host: String, number: u16, timeout: Duration, attemps: usize) -> Option<bool> {
    let mut rsl = None;
    for _ in 0..attemps {
        rsl = try_udp(host.clone(), number, timeout);
        if rsl == Some(true) {
            return rsl;
        }
    }
    rsl
}

fn try_tcp(host: String, number: u16, timeout: Duration) -> Option<bool> {
    let address = format!("{}:{}", host, number).parse::<SocketAddr>().ok()?;
    match TcpStream::connect_timeout(&address, timeout) {
        Ok(_) => Some(true),
        Err(e) => match e.kind() {
            std::io::ErrorKind::ConnectionRefused => Some(false),
            _ => None,
        },
    }
}
fn try_udp(host: String, number: u16, timeout: Duration) -> Option<bool> {
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
