use crossbeam::channel::{Receiver, Sender};
use std::thread::JoinHandle;

type Work = Option<Port>;

pub struct Scanner {
    io_channel: Receiver<char>,
    work_channel: Sender<Work>,
    output_channel: Receiver<(WorkerId, PortState)>,
    workers: Vec<JoinHandle<()>>,
}

type WorkerId = usize;

struct Worker {
    id: WorkerId,
    host: String,
    work_rx: Receiver<Work>,
    output_tx: Sender<(WorkerId, PortState)>,
}

impl Worker {
    fn tcp(&self, number: u16) -> Option<bool> {
        None
    }
    fn udp(&self, number: u16) -> Option<bool> {
        None
    }
    fn run(&self) {
        while let Some(port) = self.work_rx.recv().unwrap_or(None) {
            let output = match port.protocol {
                Protocol::Tcp => self.tcp(port.number),
                Protocol::Udp => self.udp(port.number),
            };
            let output = match output {
                Some(true) => PortState::Open,
                Some(false) => PortState::Closed,
                None => PortState::Unreachable,
            };
            match self.output_tx.send((self.id, output)) {
                Ok(_) => {}
                Err(_) => return,
            }
        }
    }
}

pub fn scan(host: String, config: ScannerConfig) {
    let (output_tx, output_rx) = crossbeam::channel::unbounded();
    let (work_tx, work_rx) = crossbeam::channel::unbounded();

    let workers = (0..config.thread_count)
        .map(|i| {
            let work_rx = work_rx.clone();
            let output_tx = output_tx.clone();
            let host = host.clone();
            std::thread::spawn(move || {
                let worker = Worker {
                    host,
                    id: i as WorkerId,
                    work_rx,
                    output_tx,
                };
                worker.run();
            })
        })
        .collect::<Vec<_>>();
}
