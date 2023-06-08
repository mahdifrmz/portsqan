use portsqan::{Input, Scanner};

fn main() {
    let scanner = Scanner::new();
    scanner.write(Input::Threads(4));
    scanner.write(Input::TcpRange("192.168.1.1".to_owned(), 80, 96));
    scanner.write(Input::Threads(8));
    // scanner.write(Input::End);
    scanner.wait();
}
