use portsqan::{Input, Scanner};

fn main() {
    let scanner = Scanner::new(|o| println!("{:?}", o));
    scanner.command(Input::Threads(64));
    scanner.command(Input::TcpRange("192.168.1.13".to_owned(), 1, 1024));
    scanner.join();
}
