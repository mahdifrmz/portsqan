use portsqan::{Input, Scanner};

fn main() {
    let scanner = Scanner::new();
    scanner.write(Input::Threads(4));
    scanner.write(Input::Threads(8));
    scanner.write(Input::End);
    scanner.destroy();
}
