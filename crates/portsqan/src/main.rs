use std::{
    process::exit,
    sync::{Arc, Mutex},
};

use rustyline::{error::ReadlineError, DefaultEditor};
use server::{Input, Output, Scanner};

enum TerminalState {
    Log,
    Repl,
}

struct Terminal {
    buffered_output: Vec<Output>,
    state: TerminalState,
}

impl Default for Terminal {
    fn default() -> Self {
        Self {
            buffered_output: vec![],
            state: TerminalState::Log,
        }
    }
}

fn print_scanner_output(output: Output) {
    println!("-> {:?}", output)
}

fn main() {
    let (int_tx, int_rx) = crossbeam::channel::bounded(1);
    let handler = move || {
        int_tx.send(()).unwrap();
    };
    ctrlc::set_handler(handler).unwrap();

    let terminal = Arc::new(Mutex::new(Terminal::default()));
    let tclone = terminal.clone();
    let scanner = Scanner::new(move |output| {
        if let Ok(mut terminal) = tclone.lock() {
            match terminal.state {
                TerminalState::Log => print_scanner_output(output),
                TerminalState::Repl => terminal.buffered_output.push(output),
            }
        }
    });
    scanner.command(Input::Threads(1));
    scanner.command(Input::UdpRange("127.0.0.1".to_owned(), 1, 1024));
    loop {
        int_rx.recv().unwrap();
        terminal.lock().unwrap().state = TerminalState::Repl;
        scanner.command(Input::Stop);
        let mut rl = DefaultEditor::new().unwrap();
        let exit = loop {
            match rl.readline("> ") {
                Ok(line) => {
                    if line.as_str() == "q" {
                        break true;
                    } else {
                        println!("ECHO: '{}'", line);
                    }
                }
                Err(ReadlineError::Eof) => break false,
                Err(ReadlineError::Interrupted) => {
                    eprintln!("ABORTING...");
                    exit(0);
                }
                Err(_) => panic!("FATAL: Failed to read STDIN"),
            }
        };
        if exit {
            break;
        }
        if let Ok(mut terminal) = terminal.lock() {
            for o in terminal.buffered_output.drain(..) {
                print_scanner_output(o);
            }
            terminal.state = TerminalState::Log;
        }
        scanner.command(Input::Cont);
    }
    scanner.command(Input::End);
    scanner.join();
}
