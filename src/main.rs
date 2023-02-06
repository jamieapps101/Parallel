use std::env::{var, VarError};
use std::io::stdin;
use std::process::Command;
use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc, Barrier,
};
use std::thread::sleep;
use std::time::Duration;
use threadpool::ThreadPool;

use clap::Parser;

/// Utility to run multiple commands in parallel. Commands are supplied via
/// stdin. The index of thread can be found using the THREAD_INDEX env var.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Execution threads to use
    #[arg(short, long)]
    threads: Option<usize>,
}

fn get_threads(args: Args) -> Option<usize> {
    if let Some(t) = args.threads {
        return Some(t);
    }

    match var("PARALLEL_THREADS") {
        Ok(string) => match string.parse::<usize>() {
            Ok(v) => return Some(v),
            Err(_r) => println!("Err: Could not parse PARALLEL_THREADS as unsigned integer"),
        },
        Err(r) => match r {
            VarError::NotPresent => {
                println!("Err: threads arg not provided and PARALLEL_THREADS var not set")
            }
            VarError::NotUnicode(_) => {
                println!("Err: threads arg not provided and PARALLEL_THREADS var not unicode")
            }
        },
    };
    None
}

#[derive(Debug)]
enum Signal {
    NextCommand(usize),
    EndThread(usize),
}

fn thread_function(
    thread_index: usize,
    barrier: Arc<Barrier>,
    req_tx: Sender<Signal>,
    command_rx: Receiver<Option<String>>,
) {
    barrier.wait();
    loop {
        // ping main thread for next command
        req_tx.send(Signal::NextCommand(thread_index)).unwrap();
        // wait for next command
        match command_rx.recv() {
            Ok(command_opt) => {
                if let Some(command) = command_opt {
                    let r = Command::new("sh")
                        .arg("-c")
                        .arg(command)
                        .env("THREAD_INDEX", format!("{thread_index}"))
                        .status();
                    if let Err(e) = r {
                        println!("Error on process: {e}");
                        break;
                    }
                } else {
                    break;
                }
            }
            Err(_) => break,
        }
        sleep(Duration::from_millis(10));
    }
    req_tx.send(Signal::EndThread(thread_index)).unwrap();
}

fn main() {
    let args = Args::parse();
    let n = if let Some(n) = get_threads(args) {
        n
    } else {
        return;
    };

    // Barrier to ensure all threads begin at the same time
    let b = Arc::new(Barrier::new(n + 1));

    // Signal to allow a thread to request the next command
    let (req_tx, req_rx) = channel::<Signal>();
    // vec of signals to allow sending work to each thread
    let mut tx_vec = vec![];

    let pool = ThreadPool::new(n);
    for i in 0..n {
        let b_local = b.clone();
        let req_tx_local = req_tx.clone();
        let (comm_tx, comm_rx) = channel::<Option<String>>();
        tx_vec.push(comm_tx);
        pool.execute(move || thread_function(i, b_local, req_tx_local, comm_rx));
    }
    // begin ingesting stdin
    let mut command_source = stdin().lines().filter_map(|l| l.ok());
    // unblock barrier
    b.wait();
    loop {
        match req_rx.recv() {
            Ok(signal) => match signal {
                Signal::NextCommand(index) => {
                    tx_vec[index].send(command_source.next()).unwrap();
                }
                Signal::EndThread(_index) => {
                    if pool.active_count() == 0 {
                        break;
                    }
                }
            },
            Err(e) => {
                panic!("Error: {e}");
            }
        }
    }
    pool.join();
}
