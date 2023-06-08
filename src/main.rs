use portsqan::{Input, Scanner};

fn main() {
    let scanner = Scanner::new(|o| println!("{:?}", o));
    scanner.command(Input::Threads(2));
    scanner.command(Input::TcpRange("192.168.1.1".to_owned(), 80, 96));
    std::thread::sleep(std::time::Duration::from_secs(4));
    scanner.command(Input::Cancel);
    scanner.command(Input::Stale(true));
    scanner.join();
}
