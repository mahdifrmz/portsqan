use std::thread::JoinHandle;

use crossbeam::channel::{Receiver, Sender};

struct WorkId(u32);

pub enum Protocol {
    Tcp,
    Udp,
}

pub struct PortRange {
    protocol: Protocol,
    from: u16,
    to: u16,
}

pub struct Port {
    protocol: Protocol,
    number: u16,
}

pub enum PortState {
    Open,
    Closed,
}

pub struct Scanner {
    io_channel: Receiver<char>,
    work_channel: Sender<(WorkId, Port)>,
    output_channel: Receiver<(WorkId, PortState)>,
    workers: Vec<JoinHandle<()>>,
}

impl Scanner {
    pub fn scan(&self, ports: Vec<PortRange>) -> Vec<(Port, PortState)> {
        vec![]
    }

    pub fn new() -> Scanner {
        let (_, io_rx) = crossbeam::channel::unbounded();
        let (_, output_rx) = crossbeam::channel::unbounded();
        let (work_tx, _) = crossbeam::channel::unbounded();

        Scanner {
            io_channel: io_rx,
            work_channel: work_tx,
            output_channel: output_rx,
            workers: vec![],
        }
    }
}
