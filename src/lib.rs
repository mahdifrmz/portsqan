use std::{thread::JoinHandle, vec};

use crossbeam::{
    channel::{Receiver, Sender},
    select,
};

#[derive(Clone, Copy)]
pub enum Protocol {
    Tcp,
    Udp,
}

pub struct PortRange {
    protocol: Protocol,
    from: u16,
    to: u16,
}

impl PortRange {
    fn len(&self) -> usize {
        (self.to - self.from + 1) as usize
    }
    fn nth(&self, index: usize) -> u16 {
        self.from + index as u16
    }
}

pub struct Port {
    protocol: Protocol,
    number: u16,
}

pub enum PortState {
    Open,
    Closed,
    Unreachable,
}

#[derive(Default)]
pub struct ScannerConfig {
    thread_count: usize,
    command_rx: Option<Receiver<Command>>,
}

impl ScannerConfig {
    pub fn thread_count(self, thread_count: usize) -> ScannerConfig {
        let mut s = self;
        s.thread_count = thread_count;
        s
    }
    pub fn command_rx(self, command_rx: Receiver<Command>) -> ScannerConfig {
        let mut s = self;
        s.command_rx = Some(command_rx);
        s
    }
}

struct PortIter {
    range_index: usize,
    port_index: usize,
    ranges: Vec<PortRange>,
}

impl PortIter {
    fn new(ranges: Vec<PortRange>) -> PortIter {
        PortIter {
            range_index: 0,
            port_index: 0,
            ranges,
        }
    }
    fn next(&mut self) -> Option<Port> {
        let range = &self.ranges[self.range_index];
        if self.port_index == range.len() {
            if self.range_index == self.ranges.len() - 1 {
                None
            } else {
                self.port_index = 0;
                self.range_index += 1;
                self.next()
            }
        } else {
            let port = Port {
                protocol: range.protocol,
                number: range.nth(self.port_index),
            };
            self.port_index += 1;
            Some(port)
        }
    }
}

enum Instruction {
    Host(String),
    Port(Port),
    Term,
}

type WorkerId = usize;

struct WorkerMessage {
    worker_id: WorkerId,
    content: Message,
}

enum Message {
    Quit,
    Scan(PortState),
    Error,
}

struct Worker {
    id: WorkerId,
    work_rx: Receiver<Instruction>,
    message_tx: Sender<WorkerMessage>,
}

impl Worker {
    fn send_message(&self, message: Message) {
        self.message_tx
            .send(WorkerMessage {
                worker_id: self.id,
                content: message,
            })
            .unwrap();
    }
    fn tcp(&self, host: String, number: u16) -> Option<bool> {
        None
    }
    fn udp(&self, host: String, number: u16) -> Option<bool> {
        None
    }
    fn run(&self) {
        loop {
            let mut host = None;
            match self.work_rx.recv().unwrap_or(Instruction::Term) {
                Instruction::Host(h) => host = Some(h),
                Instruction::Port(port) => {
                    let host = match host {
                        Some(host) => host,
                        None => {
                            self.send_message(Message::Error);
                            break;
                        }
                    };
                    let scan = match port.protocol {
                        Protocol::Tcp => self.tcp(host, port.number),
                        Protocol::Udp => self.udp(host, port.number),
                    };
                    let scan = match scan {
                        Some(true) => PortState::Open,
                        Some(false) => PortState::Closed,
                        None => PortState::Unreachable,
                    };
                    self.send_message(Message::Scan(scan));
                }
                Instruction::Term => {
                    self.send_message(Message::Quit);
                    break;
                }
            }
        }
    }
}

pub enum Command {
    Stop,
    Cont,
}

struct Scanner {
    worker_tx: Vec<Sender<Instruction>>,
    message_rx: Receiver<WorkerMessage>,
    command_rx: Receiver<Command>,
    ranges: PortIter,
    config: ScannerConfig,
    handles: Vec<JoinHandle<()>>,
}

impl Scanner {
    fn new(ranges: Vec<PortRange>, config: ScannerConfig) -> Scanner {
        let (message_tx, message_rx) = crossbeam::channel::unbounded();
        let command_rx = config
            .command_rx
            .clone()
            .unwrap_or(crossbeam::channel::bounded(1).1);
        let worker_count = config.thread_count as usize;
        let mut worker_tx = vec![];
        let handles = (0..worker_count)
            .map(|i| {
                let (work_tx, work_rx) = crossbeam::channel::unbounded();
                let message_tx = message_tx.clone();
                worker_tx.push(work_tx);
                std::thread::spawn(move || {
                    let worker = Worker {
                        id: i as WorkerId,
                        work_rx,
                        message_tx,
                    };
                    worker.run();
                })
            })
            .collect::<Vec<_>>();

        let ranges = PortIter::new(ranges);
        Scanner {
            command_rx,
            worker_tx,
            message_rx,
            ranges,
            config,
            handles,
        }
    }
    fn send_instruction(&self, worker_id: WorkerId, instruction: Instruction) {
        self.worker_tx[worker_id].send(instruction).unwrap();
    }
    fn host(&mut self, host: String) {
        for i in 0..self.config.thread_count {
            self.send_instruction(i, Instruction::Host(host.clone()));
        }
    }
    fn terminate(&mut self) {
        for i in 0..self.config.thread_count {
            self.send_instruction(i, Instruction::Term);
        }
    }
    fn join(&mut self) {
        for h in self.handles.drain(..) {
            h.join().unwrap();
        }
    }
    fn handle_message(&mut self, message: WorkerMessage) -> bool {
        false
    }
    fn handle_command(&mut self, command: Command) -> bool {
        false
    }
    fn run(&mut self) {
        let message_rx = self.message_rx.clone();
        let command_rx = self.command_rx.clone();
        let mut term = false;
        while !term {
            select! {
                recv(message_rx) -> message => term = self.handle_message(message.unwrap()),
                recv(command_rx) -> command => term = self.handle_command(command.unwrap()),
            };
        }
    }
}

pub fn scan(host: String, ranges: Vec<PortRange>, config: ScannerConfig) {
    let mut scanner = Scanner::new(ranges, config);
    scanner.host(host);
    scanner.run();
    scanner.terminate();
    scanner.join();
}
