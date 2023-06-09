use server::{Input, Output, Scanner};

#[derive(Default)]
pub struct ScannerBuilder {
    thread_count: Option<usize>,
    tcp_timeout: Option<usize>,
    udp_timeout: Option<usize>,
    attemps: Option<usize>,
    stale: Option<bool>,
}

impl ScannerBuilder {
    pub fn thread_count(self, value: usize) -> Self {
        let mut s = self;
        s.thread_count = Some(value);
        s
    }
    pub fn attemps(self, value: usize) -> Self {
        let mut s = self;
        s.attemps = Some(value);
        s
    }
    pub fn tcp_timeout(self, value: usize) -> Self {
        let mut s = self;
        s.tcp_timeout = Some(value);
        s
    }
    pub fn udp_timeout(self, value: usize) -> Self {
        let mut s = self;
        s.udp_timeout = Some(value);
        s
    }
    pub fn stale(self, value: bool) -> Self {
        let mut s = self;
        s.stale = Some(value);
        s
    }
    fn config(&self, scanner: &Scanner) {
        if let Some(val) = self.attemps {
            scanner.command(Input::Attmpts(val));
        }
        if let Some(val) = self.stale {
            scanner.command(Input::Stale(val));
        }
        if let Some(val) = self.thread_count {
            scanner.command(Input::Threads(val));
        }
        if let Some(val) = self.tcp_timeout {
            scanner.command(Input::TcpTimeout(val));
        }
        if let Some(val) = self.udp_timeout {
            scanner.command(Input::UdpTimeout(val));
        }
    }
    pub fn build<O: Fn(Output) + Send + Clone + 'static>(self, o: O) -> Scanner {
        let scanner = Scanner::new(o);
        self.config(&scanner);
        scanner
    }
    pub fn run(self) -> Vec<Output> {
        let mut output = vec![];
        let (tx, rx) = crossbeam::channel::unbounded();
        self.build(move |o| {
            let _ = tx.send(o);
        });
        loop {
            let o = rx.recv().unwrap();
            if o == Output::Idle {
                break;
            } else {
                output.push(o)
            }
        }
        output
    }
}
