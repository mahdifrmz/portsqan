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
    input_rx: Option<Receiver<Input>>,
}

impl ScannerConfig {
    pub fn thread_count(self, thread_count: usize) -> ScannerConfig {
        let mut s = self;
        s.thread_count = thread_count;
        s
    }
    pub fn input_rx(self, input_rx: Receiver<Input>) -> ScannerConfig {
        let mut s = self;
        s.input_rx = Some(input_rx);
        s
    }
}

#[derive(Default)]
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
    Scan(Port, PortState),
    Error,
}

struct Worker {
    id: WorkerId,
    work_rx: Receiver<Instruction>,
    message_tx: Sender<WorkerMessage>,
}

struct WorkerHandle {
    id: WorkerId,
    state: WorkerState,
    work_tx: Sender<Instruction>,
    join_handle: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    fn is_idle(&self) -> bool {
        self.state == WorkerState::Idle
    }
    fn is_working(&self) -> bool {
        self.state == WorkerState::Working
    }
    fn join(&mut self) {
        if let Some(h) = self.join_handle.take() {
            h.join()
                .expect(format!("FATAL: Worker #{} has paniced!", self.id).as_str());
        }
    }
    fn send_instruction(&self, instruction: Instruction) {
        self.work_tx.send(instruction).expect(
            "FATAL: Scanner failed to send instruction. \
        The thread has been probably terminated too early, or either the instruction is late.",
        )
    }
}

#[derive(PartialEq, Eq)]
enum WorkerState {
    Term,
    Working,
    Idle,
}
#[derive(PartialEq, Eq)]
enum ScannerState {
    Ending,
    Terminated,
    Stop,
    Running,
}

impl Worker {
    fn send_message(&self, message: Message) {
        self.message_tx
            .send(WorkerMessage {
                worker_id: self.id,
                content: message,
            })
            .expect(
                "FATAL: Worker thread failed to send message. \
The channel has been probably closed by the scanner too early.",
            );
    }
    fn tcp(&self, host: String, number: u16) -> Option<bool> {
        todo!()
    }
    fn udp(&self, host: String, number: u16) -> Option<bool> {
        todo!()
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
                    self.send_message(Message::Scan(port, scan));
                }
                Instruction::Term => {
                    break;
                }
            }
        }
    }
}

pub enum Input {
    Stop,
    Cont,
    End,
}

struct Scanner {
    workers: Vec<WorkerHandle>,
    message_rx: Receiver<WorkerMessage>,
    input_rx: Receiver<Input>,
    ranges: PortIter,
    config: ScannerConfig,
    state: ScannerState,
    output: Vec<(Port, PortState)>,
}

impl Scanner {
    fn new(ranges: Vec<PortRange>, config: ScannerConfig) -> Scanner {
        let (message_tx, message_rx) = crossbeam::channel::unbounded();
        let input_rx = config
            .input_rx
            .clone()
            .unwrap_or(crossbeam::channel::never());
        let worker_count = config.thread_count as usize;
        let workers = (0..worker_count)
            .map(|id| {
                let (work_tx, work_rx) = crossbeam::channel::bounded(1);
                let message_tx = message_tx.clone();
                WorkerHandle {
                    id,
                    work_tx,
                    state: WorkerState::Working,
                    join_handle: Some(std::thread::spawn(move || {
                        let worker = Worker {
                            id,
                            work_rx,
                            message_tx,
                        };
                        worker.run();
                    })),
                }
            })
            .collect::<Vec<_>>();

        let ranges = PortIter::new(ranges);

        Scanner {
            workers,
            input_rx,
            message_rx,
            ranges,
            config,
            state: ScannerState::Running,
            output: vec![],
        }
    }
    fn host(&mut self, host: String) {
        for wh in self.workers.iter_mut() {
            wh.send_instruction(Instruction::Host(host.clone()));
        }
    }
    fn try_terminate(&mut self) -> bool {
        let mut success = true;
        for wh in self.workers.iter_mut() {
            if wh.is_idle() {
                wh.state = WorkerState::Term;
                wh.send_instruction(Instruction::Term);
                wh.join();
            } else if wh.is_working() {
                success = false;
            }
        }
        return success;
    }
    fn handle_message(&mut self, message: WorkerMessage) {
        match message.content {
            Message::Scan(port, state) => {
                let worker_id = message.worker_id;
                self.workers[worker_id].state = WorkerState::Idle;
                self.output.push((port, state));
                if self.state == ScannerState::Running {
                    self.assign_work();
                } else if self.state == ScannerState::Ending {
                    self.try_terminate();
                }
            }
            Message::Error => panic!("server assigned jobs before setting a Host"),
        }
    }
    fn handle_input(&mut self, input: Input) {
        match input {
            Input::End => self.state = ScannerState::Ending,
            Input::Stop => {
                if self.state == ScannerState::Running {
                    self.state = ScannerState::Stop;
                }
            }
            Input::Cont => {
                if self.state == ScannerState::Stop {
                    self.state = ScannerState::Running;
                }
                self.assign_work()
            }
        }
    }
    fn assign_work(&mut self) {
        let mut ranges = std::mem::take(&mut self.ranges);
        for wh in self.workers.iter_mut() {
            if wh.is_idle() {
                match ranges.next() {
                    Some(port) => {
                        wh.send_instruction(Instruction::Port(port));
                        wh.state = WorkerState::Working;
                    }
                    None => todo!(),
                }
            }
        }
        self.ranges = ranges;
    }
    fn drop_input_channel(&mut self) {
        self.input_rx = crossbeam::channel::never();
    }
    fn listen(&mut self) -> Vec<(Port, PortState)> {
        let message_rx = self.message_rx.clone();
        let input_rx = self.input_rx.clone();
        while self.state != ScannerState::Terminated {
            select! {
                recv(message_rx) -> message => self.handle_message(message.expect(
                    "FATAL: Scanner failed to receive message. \
            The thread has been probably terminated too early, or either the recv call is late.")),
                recv(input_rx) -> input => match input {
                    Err(_) => self.drop_input_channel(),
                    Ok(input) => self.handle_input(input),
                },
            };
        }
        vec![]
    }
}

pub fn scan(host: String, ranges: Vec<PortRange>, config: ScannerConfig) -> Vec<(Port, PortState)> {
    let mut scanner = Scanner::new(ranges, config);
    scanner.host(host);
    let ouput = scanner.listen();
    ouput
}
