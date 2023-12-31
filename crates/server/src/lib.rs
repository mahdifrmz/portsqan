mod net;

use std::{
    sync::{Arc, Mutex, MutexGuard},
    thread::JoinHandle,
    time::Duration,
    vec,
};

use crossbeam::{
    channel::{Receiver, Sender},
    select,
};

#[derive(Clone, Copy)]
pub enum Protocol {
    Tcp,
    Udp,
}

pub struct AddressRange {
    host: String,
    protocol: Protocol,
    from: u16,
    to: u16,
}

impl AddressRange {
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

#[derive(PartialEq, Eq, Debug)]
pub enum PortState {
    Open,
    Closed,
    Unreachable,
}

pub struct ScannerConfig {
    thread_count: usize,
    stale: bool,
    tcp_timeout: usize, // miliseconds
    udp_timeout: usize, // miliseconds
    attemps: usize,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            attemps: 1,
            thread_count: 1,
            stale: true,
            tcp_timeout: 500,
            udp_timeout: 500,
        }
    }
}

impl ScannerConfig {
    pub fn thread_count(self, thread_count: usize) -> ScannerConfig {
        let mut s = self;
        s.thread_count = thread_count;
        s
    }
}

#[derive(Default)]
struct ScanQueue {
    address_index: usize,
    ranges: Vec<AddressRange>,
}

impl ScanQueue {
    fn new() -> ScanQueue {
        ScanQueue {
            address_index: 0,
            ranges: vec![],
        }
    }
    fn pop(&mut self) -> Option<Address> {
        if let Some(address_range) = self.ranges.first() {
            if address_range.len() > self.address_index {
                let number = address_range.nth(self.address_index);
                self.address_index += 1;
                let address = (
                    address_range.host.clone(),
                    Port {
                        protocol: address_range.protocol,
                        number,
                    },
                );
                Some(address)
            } else {
                self.ranges.remove(0);
                self.address_index = 0;
                self.pop()
            }
        } else {
            None
        }
    }
    fn push(&mut self, address_range: AddressRange) {
        self.ranges.push(address_range)
    }
    fn len(&self) -> usize {
        self.ranges.len()
    }

    fn clear(&mut self) {
        self.ranges.clear();
        self.address_index = 0;
    }
}

type Host = String;

type Address = (Host, Port);

enum Instruction {
    Scan(Address),
    Term,
}

type WorkerId = usize;

struct WorkerMessage {
    worker_id: WorkerId,
    content: Message,
}

enum Message {
    Scan(Host, Port, PortState),
}

struct Worker {
    id: WorkerId,
    work_rx: Receiver<Instruction>,
    message_tx: Sender<WorkerMessage>,
    config: Arc<Mutex<ScannerConfig>>,
}

struct WorkerHandle {
    stale: bool,
    id: WorkerId,
    state: WorkerState,
    work_tx: Sender<Instruction>,
    join_handle: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    fn is_idle(&self) -> bool {
        self.state == WorkerState::Idle
    }
    fn is_term(&self) -> bool {
        self.state == WorkerState::Term
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
    fn config(&self) -> MutexGuard<ScannerConfig> {
        self.config.lock().unwrap()
    }
    fn tcp(&self, host: String, number: u16) -> Option<bool> {
        let config = self.config();
        let attemps = config.attemps;
        let timeout = config.tcp_timeout;
        drop(config);
        net::scan_tcp(host, number, Duration::from_millis(timeout as u64), attemps)
    }
    fn udp(&self, host: String, number: u16) -> Option<bool> {
        let config = self.config();
        let attemps = config.attemps;
        let timeout = config.udp_timeout;
        drop(config);
        net::scan_udp(host, number, Duration::from_millis(timeout as u64), attemps)
    }
    fn run(&self) {
        loop {
            match self.work_rx.recv().unwrap_or(Instruction::Term) {
                Instruction::Scan((host, port)) => {
                    let scan = match port.protocol {
                        Protocol::Tcp => self.tcp(host.clone(), port.number),
                        Protocol::Udp => self.udp(host.clone(), port.number),
                    };
                    let scan = match scan {
                        Some(true) => PortState::Open,
                        Some(false) => PortState::Closed,
                        None => PortState::Unreachable,
                    };
                    self.send_message(Message::Scan(host, port, scan));
                }
                Instruction::Term => {
                    break;
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum Input {
    Stop,
    Cont,
    End,
    TcpRange(String, u16, u16),
    UdpRange(String, u16, u16),
    Threads(usize),
    Stale(bool),
    Cancel,
    NOP,
    Ping,
    Attmpts(usize),
    TcpTimeout(usize),
    UdpTimeout(usize),
}

#[derive(PartialEq, Eq, Debug)]
pub enum Output {
    // async
    TcpScan(String, u16, PortState),
    UdpScan(String, u16, PortState),
    Idle,
    // sync
    Ok,
}
struct ScanMaster<O: Fn(Output)> {
    workers: Vec<WorkerHandle>,
    message_rx: Receiver<WorkerMessage>,
    message_tx: Sender<WorkerMessage>,
    ranges: ScanQueue,
    config: Arc<Mutex<ScannerConfig>>,
    state: ScannerState,
    input_rx: Receiver<Input>,
    output_tx: Sender<Output>,
    output: O,
    id_counter: usize,
}

impl<O: Fn(Output)> ScanMaster<O> {
    fn new(output: O, input_rx: Receiver<Input>, output_tx: Sender<Output>) -> ScanMaster<O> {
        let (message_tx, message_rx) = crossbeam::channel::unbounded();
        let workers = vec![];
        let ranges = ScanQueue::new();
        ScanMaster {
            workers,
            message_rx,
            message_tx,
            ranges,
            config: Arc::new(Mutex::new(ScannerConfig::default())),
            state: ScannerState::Running,
            input_rx,
            output,
            output_tx,
            id_counter: 0,
        }
    }
    fn send_async_output(&self, output: Output) {
        let cb = &self.output;
        cb(output)
    }
    fn send_sync_output(&self, output: Output) {
        let _ = self.output_tx.send(output);
    }
    fn threads_clean(&mut self) {
        self.workers = self
            .workers
            .drain(..)
            .filter(|wh| !wh.is_term())
            .collect::<Vec<_>>();
    }
    fn try_close(&mut self, count: usize) {
        let mut count = count;
        for wh in self.workers.iter_mut() {
            if count == 0 {
                break;
            }
            if wh.is_idle() {
                wh.state = WorkerState::Term;
                wh.send_instruction(Instruction::Term);
                wh.join();
                count -= 1;
            }
        }
        self.threads_clean();
    }
    fn try_terminate(&mut self) {
        self.try_close(self.workers.len());
        if self.workers.len() == 0 {
            self.state = ScannerState::Terminated;
        }
    }
    fn config(&mut self) -> MutexGuard<ScannerConfig> {
        self.config.lock().unwrap()
    }
    fn thread_count_control(&mut self) {
        let expected_count = self.config().thread_count;
        if expected_count > self.workers.len() {
            let diff = expected_count - self.workers.len();
            for _ in 0..diff {
                self.spawn();
            }
            self.assign_work();
        } else if expected_count < self.workers.len() {
            let diff = self.workers.len() - expected_count;
            self.try_close(diff)
        }
    }
    fn handle_message(&mut self, message: WorkerMessage) {
        match message.content {
            Message::Scan(host, port, state) => {
                let worker_id = message.worker_id;
                let worker_idx = self
                    .workers
                    .binary_search_by_key(&worker_id, |wh| wh.id)
                    .unwrap();
                let worker = &mut self.workers[worker_idx];
                worker.state = WorkerState::Idle;
                let stale = worker.stale;
                worker.stale = false;
                if !stale || !self.config().stale {
                    match port.protocol {
                        Protocol::Tcp => {
                            self.send_async_output(Output::TcpScan(host, port.number, state))
                        }
                        Protocol::Udp => {
                            self.send_async_output(Output::UdpScan(host, port.number, state))
                        }
                    }
                }
                if self.state == ScannerState::Running {
                    self.thread_count_control();
                    self.assign_work();
                    self.check_idle();
                } else if self.state == ScannerState::Ending {
                    self.try_terminate();
                }
            }
        }
    }
    fn stale_all(&mut self) {
        for wh in self.workers.iter_mut() {
            wh.stale = true;
        }
    }
    fn handle_input(&mut self, input: Input) {
        if self.state == ScannerState::Ending || self.state == ScannerState::Terminated {
            return;
        }
        match input {
            Input::End => {
                self.state = ScannerState::Ending;
                self.stale_all();
                self.try_terminate();
            }
            Input::Ping => {}
            Input::Attmpts(count) => {
                self.config().attemps = count;
            }
            Input::TcpTimeout(milis) => {
                self.config().tcp_timeout = milis;
            }
            Input::UdpTimeout(milis) => {
                self.config().udp_timeout = milis;
            }
            Input::Cancel => {
                self.stale_all();
                self.ranges.clear();
            }
            Input::Stale(stale) => {
                self.config().stale = stale;
            }
            Input::TcpRange(host, from, to) => {
                self.ranges.push(AddressRange {
                    host,
                    protocol: Protocol::Tcp,
                    from,
                    to,
                });
                self.assign_work();
            }
            Input::UdpRange(host, from, to) => {
                self.ranges.push(AddressRange {
                    host,
                    protocol: Protocol::Udp,
                    from,
                    to,
                });
                self.assign_work();
            }
            Input::Stop => {
                if self.state == ScannerState::Running {
                    self.state = ScannerState::Stop;
                }
            }
            Input::Cont => {
                if self.state == ScannerState::Stop {
                    self.state = ScannerState::Running;
                }
                self.assign_work();
            }
            Input::Threads(count) => {
                self.config().thread_count = count;
                self.thread_count_control();
            }
            Input::NOP => {}
        }
        self.send_sync_output(Output::Ok);
    }
    fn spawn(&mut self) {
        self.id_counter += 1;
        let id = self.id_counter;
        let (work_tx, work_rx) = crossbeam::channel::bounded(1);
        let message_tx = self.message_tx.clone();
        let config = self.config.clone();
        let handle = WorkerHandle {
            id: self.id_counter,
            work_tx,
            state: WorkerState::Idle,
            join_handle: Some(std::thread::spawn(move || {
                let worker = Worker {
                    id,
                    work_rx,
                    message_tx,
                    config,
                };
                worker.run();
            })),
            stale: false,
        };
        self.workers.push(handle);
    }
    fn assign_work(&mut self) {
        if self.state != ScannerState::Running {
            return;
        }
        let mut ranges = std::mem::take(&mut self.ranges);
        for wh in self.workers.iter_mut() {
            if wh.is_idle() {
                if let Some(address) = ranges.pop() {
                    wh.send_instruction(Instruction::Scan(address));
                    wh.state = WorkerState::Working;
                } else {
                    break;
                }
            }
        }
        self.ranges = ranges;
    }
    fn check_idle(&self) {
        if self.workers.iter().filter(|wh| wh.is_idle()).count() == self.workers.len()
            && self.state == ScannerState::Running
            && self.ranges.len() == 0
        {
            self.send_async_output(Output::Idle)
        }
    }
    fn drop_input_channel(&mut self) {
        self.input_rx = crossbeam::channel::never();
    }
    fn listen(&mut self) {
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
    }
}

#[derive(Clone)]
pub struct Scanner {
    tx: Sender<Input>,
    rx: Receiver<Output>,
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl Scanner {
    pub fn new<O: Fn(Output) + Send + 'static>(output: O) -> Scanner {
        let (input_tx, input_rx) = crossbeam::channel::unbounded();
        let (output_tx, output_rx) = crossbeam::channel::unbounded();
        let mut scan_master = ScanMaster::new(output, input_rx, output_tx);
        let handle = std::thread::spawn(move || {
            scan_master.thread_count_control();
            scan_master.listen();
        });
        let handle = Arc::new(Mutex::new(Some(handle)));
        Scanner {
            tx: input_tx,
            rx: output_rx,
            handle,
        }
    }
    pub fn command(&self, input: Input) -> Option<Output> {
        self.tx.send(input).ok()?;
        self.rx.recv().ok()
    }
    pub fn join(&self) -> Option<()> {
        self.handle.lock().ok()?.take()?.join().ok()
    }
}
