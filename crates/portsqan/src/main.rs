use std::{
    process::exit,
    sync::{Arc, Mutex},
};

use libportsqan::ScannerBuilder;
use parser::{Parser, ReplConfig};
use rustyline::{error::ReadlineError, DefaultEditor, ExternalPrinter};
use server::{Input, Output};

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

fn print_scanner_output<P: ExternalPrinter>(printer: Arc<Mutex<P>>, output: Output) {
    let _ = printer.lock().unwrap().print(format!("| {:?}\n", output));
}

fn run_server(config: ScannerBuilder) {
    let (int_tx, int_rx) = crossbeam::channel::bounded(1);
    let handler = move || {
        int_tx.send(()).unwrap();
    };
    ctrlc::set_handler(handler).unwrap();

    let terminal = Arc::new(Mutex::new(Terminal::default()));
    let tclone = terminal.clone();
    let mut rl = DefaultEditor::new().unwrap();
    let printer = Arc::new(Mutex::new(rl.create_external_printer().unwrap()));
    let pclone = printer.clone();
    let scanner = config.build(move |output| {
        if let Ok(mut terminal) = tclone.lock() {
            match terminal.state {
                TerminalState::Log => print_scanner_output(pclone.clone(), output),
                TerminalState::Repl => terminal.buffered_output.push(output),
            }
        }
    });
    let mut state = ReplConfig {
        host: None,
        autostop: false,
    };
    let mut parser = Parser::default();

    loop {
        int_rx.recv().unwrap();
        if state.autostop {
            terminal.lock().unwrap().state = TerminalState::Repl;
            scanner.command(Input::Stop);
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
                                    Input::NOP => {}
                                    Input::End => break true,
                                    Input::Cancel => {
                                        terminal.lock().unwrap().clear_scan_results();
                                        if let Some(output) = scanner.command(input) {
                                            print_scanner_output(printer.clone(), output)
                                        }
                                    }
                                    _ => {
                                        if let Some(output) = scanner.command(input) {
                                            print_scanner_output(printer.clone(), output)
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
                print_scanner_output(printer.clone(), o);
            }
            terminal.state = TerminalState::Log;
        }
        scanner.command(Input::Cont);
    }
    scanner.command(Input::End);
    scanner.join();
}

fn main() {
    run_server(
        ScannerBuilder::default()
            .thread_count(1)
            .tcp_timeout(1000)
            .scan_tcp("116.203.221.27".to_owned(), 1, 10)
            .stale(true)
            .attemps(1),
    )
}
