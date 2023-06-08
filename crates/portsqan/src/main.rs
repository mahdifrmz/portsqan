use std::{
    process::exit,
    sync::{Arc, Mutex},
};

use parser::{Parser, ReplState};
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
impl Terminal {
    fn clear_scan_results(&mut self) {
        self.buffered_output = self
            .buffered_output
            .drain(..)
            .filter(|s| match s {
                Output::TcpScan(_, _, _) | Output::UdpScan(_, _, _) => false,
                _ => true,
            })
            .collect::<Vec<_>>()
    }
}

fn print_scanner_output(output: Output) {
    println!("| {:?}", output)
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
    let mut state = ReplState { host: None };
    let mut parser = Parser::default();
    let mut rl = DefaultEditor::new().unwrap();
    loop {
        int_rx.recv().unwrap();
        terminal.lock().unwrap().state = TerminalState::Repl;
        scanner.command(Input::Stop);
        let exit = loop {
            let prompt = format!("{}> ", state.host.clone().unwrap_or("".to_owned()));
            match rl.readline(prompt.as_str()) {
                Ok(line) => {
                    let _ = rl.add_history_entry(&line);
                    if line.trim().len() > 0 {
                        let (rsl, new_state) = parser.parse(state, line);
                        state = new_state;
                        match rsl {
                            Ok(input) => {
                                match input {
                                    Input::NOP => {}
                                    Input::Cont => break false,
                                    Input::End => break true,
                                    Input::Cancel => {
                                        terminal.lock().unwrap().clear_scan_results();
                                        if let Some(output) = scanner.command(input) {
                                            print_scanner_output(output)
                                        }
                                    }
                                    _ => {
                                        if let Some(output) = scanner.command(input) {
                                            print_scanner_output(output)
                                        }
                                    }
                                };
                            }
                            Err(err) => eprintln!("Error: {:?}", err),
                        }
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
