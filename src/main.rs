use portsqan::{Input, Scanner};

fn main() {
    let scanner = Scanner::new(|o| println!("{:?}", o));
    scanner.command(Input::Threads(4));
    scanner.command(Input::TcpRange("192.168.1.1".to_owned(), 80, 96));
    scanner.command(Input::Threads(8));
    // scanner.write(Input::End);
    scanner.join();
}
