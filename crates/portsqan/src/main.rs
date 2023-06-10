mod repl;

use std::process::exit;

use clap::{command, Parser};
use libportsqan::ScannerBuilder;
use repl::run_repl;

#[derive(Parser)]
#[command(name = "Portsqan")]
#[command(version = "0.1.0")]
#[command(about = "Port scanning utility")]
struct CliArgs {
    host: String,
    // scan
    #[clap(long, short)]
    from: Option<u16>,

    #[clap(long, short)]
    to: Option<u16>,

    #[clap(long, short)]
    protocol: Option<String>,

    #[clap(long)]
    exclude_from: Option<u16>,

    #[clap(long)]
    exclude_to: Option<u16>,
    // config
    #[clap(long)]
    thread_count: Option<usize>,

    #[clap(long)]
    tcp_timeout: Option<usize>,

    #[clap(long)]
    udp_timeout: Option<usize>,

    #[clap(long)]
    attemps: Option<usize>,

    #[clap(long)]
    stale: Option<bool>,
}

fn zero_port_check(port: u16) {
    if port == 0 {
        eprintln!("ERROR: port number must be in (1,65535)!");
        exit(1);
    }
}
fn invalid_range() -> ! {
    eprintln!("invalid port range");
    exit(1)
}
fn invalid_exclusion() -> ! {
    eprintln!("invalid exclusion range");
    exit(1)
}
fn check_range(from: u16, to: u16) {
    zero_port_check(from);
    zero_port_check(to);
    if to < from {
        invalid_range();
    }
}

fn main() {
    let mut builder = ScannerBuilder::default();
    let args = CliArgs::parse();
    if let Some(value) = args.thread_count {
        builder = builder.thread_count(value);
    }
    if let Some(value) = args.tcp_timeout {
        builder = builder.tcp_timeout(value);
    }
    if let Some(value) = args.udp_timeout {
        builder = builder.udp_timeout(value);
    }
    if let Some(value) = args.attemps {
        builder = builder.attemps(value);
    }
    if let Some(value) = args.stale {
        builder = builder.stale(value);
    }

    let is_tcp = args
        .protocol
        .unwrap_or("tcp".to_owned())
        .to_lowercase()
        .as_str()
        == "tcp";

    let from = args.from.unwrap_or(1);
    let to = args.to.unwrap_or(0xffff);
    check_range(from, to);

    let mut ranges = if let Some((ex_f, ex_t)) = match (args.exclude_from, args.exclude_to) {
        (None, None) => None,
        (Some(f), Some(t)) => Some((f, t)),
        _ => invalid_exclusion(),
    } {
        check_range(ex_f, ex_t);
        range(from, to, ex_f, ex_t)
    } else {
        vec![(from, to)]
    };

    for r in ranges.drain(..) {
        let (from, to) = r;
        if is_tcp {
            builder = builder.scan_tcp(args.host.clone(), from, to);
        } else {
            builder = builder.scan_udp(args.host.clone(), from, to);
        }
    }

    run_repl(builder);
}

fn range(f: u16, t: u16, xf: u16, xt: u16) -> Vec<(u16, u16)> {
    let mut ranges = vec![];
    let mut marker = 0;
    let mut state = false;
    for i in f..t {
        if i > xt || i < xf {
            if !state {
                state = true;
                marker = i;
            }
        } else {
            if state {
                state = false;
                ranges.push((marker, i - 1));
            }
        }
    }
    if state {
        ranges.push((marker, t))
    }
    ranges
}
