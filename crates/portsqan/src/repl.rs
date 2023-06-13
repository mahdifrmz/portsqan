use std::{
    process::exit,
    sync::{Arc, Mutex},
};

use libportsqan::ScannerBuilder;
use parser::{Parser, ReplConfig};
use rustyline::{error::ReadlineError, DefaultEditor, ExternalPrinter};
use server::{Input, Output, Scanner};

enum TerminalState {
    Log,
    Store,
}

struct Terminal<P: ExternalPrinter> {
    buffered_output: Vec<Output>,
    state: TerminalState,
    printer: P,
}

impl<P: ExternalPrinter> Terminal<P> {
    fn new(printer: P) -> Self {
        Self {
            buffered_output: vec![],
            state: TerminalState::Log,
            printer,
        }
    }

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

    fn flush(&mut self) {
        while self.buffered_output.len() > 0 {
            let o = self.buffered_output.remove(0);
            self.print(o);
        }
    }

    fn print(&mut self, output: Output) {
        let _ = self.printer.print(format!("| {:?}\n", output));
    }
}

fn stop<P: ExternalPrinter>(scanner: &Scanner, terminal: Arc<Mutex<Terminal<P>>>, silent: bool) {
    if let Ok(mut terminal) = terminal.lock() {
        terminal.state = TerminalState::Store;
        if let Some(output) = scanner.command(Input::Stop) {
            if !silent {
                terminal.print(output);
            }
        }
    }
}

fn resume<P: ExternalPrinter>(scanner: &Scanner, terminal: Arc<Mutex<Terminal<P>>>, silent: bool) {
    if let Ok(mut terminal) = terminal.lock() {
        terminal.state = TerminalState::Log;
        terminal.flush();
        if let Some(output) = scanner.command(Input::Cont) {
            if !silent {
                terminal.print(output);
            }
        }
    }
}

pub fn run_repl(config: ScannerBuilder, host: String) {
    let (int_tx, int_rx) = crossbeam::channel::bounded(1);
    let handler = move || {
        int_tx.send(()).unwrap();
    };
    ctrlc::set_handler(handler).unwrap();

    let mut rl = DefaultEditor::new().unwrap();
    let terminal = Arc::new(Mutex::new(Terminal::new(
        rl.create_external_printer().unwrap(),
    )));
    let tclone = terminal.clone();
    let scanner = config.build(move |output| {
        if let Ok(mut terminal) = tclone.lock() {
            match terminal.state {
                TerminalState::Log => terminal.print(output),
                TerminalState::Store => terminal.buffered_output.push(output),
            }
        }
    });
    let mut state = ReplConfig {
        host: Some(host),
        autostop: true,
    };
    let mut parser = Parser::default();

    loop {
        int_rx.recv().unwrap();
        if state.autostop {
            stop(&scanner, terminal.clone(), true);
        }
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
                                    Input::Stop => stop(&scanner, terminal.clone(), false),
                                    Input::Cont => resume(&scanner, terminal.clone(), false),
                                    Input::End => break true,
                                    Input::Cancel => {
                                        if let Ok(mut terminal) = terminal.lock() {
                                            terminal.clear_scan_results();
                                            if let Some(output) = scanner.command(input) {
                                                terminal.print(output)
                                            }
                                        }
                                    }
                                    _ => {
                                        if let Ok(mut terminal) = terminal.lock() {
                                            if let Some(output) = scanner.command(input) {
                                                terminal.print(output)
                                            }
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
        resume(&scanner, terminal.clone(), true)
    }
    scanner.command(Input::End);
    scanner.join();
}
