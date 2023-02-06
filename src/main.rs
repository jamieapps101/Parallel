use std::env::{var, VarError};
use std::io::stdin;
use std::process::Command;
use std::sync::{
    mpsc::{channel, sync_channel, Receiver, Sender},
    Arc, Barrier, LockResult, Mutex,
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
    NextCommand,
    EndThread,
}

fn thread_function(
    thread_index: usize,
    barrier: Arc<Barrier>,
    req_tx: Sender<Signal>,
    command_rx: Arc<Mutex<Receiver<Option<String>>>>,
) {
    barrier.wait();
    loop {
        // ping main thread for next command
        println!("Thread {thread_index}) request command");
        req_tx.send(Signal::NextCommand).unwrap();
        // wait for next command
        match command_rx.lock() {
            LockResult::Ok(rx) => match rx.recv() {
                Ok(command_opt) => {
                    if let Some(command) = command_opt {
                        println!("Thread {thread_index}) running");
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
            },
            LockResult::Err(_) => break,
        }
        sleep(Duration::from_millis(10));
    }
    req_tx.send(Signal::EndThread).unwrap();
}

fn main() {
    let args = Args::parse();
    let n = if let Some(n) = get_threads(args) {
        n
    } else {
        return;
    };

    println!("n = {n}");
    let b = Arc::new(Barrier::new(n + 1));

    // Signal to allow a thread to request the next command
    let (req_tx, req_rx) = channel::<Signal>();

    // Signal to send next command
    let (comm_tx, comm_rx) = sync_channel::<Option<String>>(5);
    let comm_rx_am = Arc::new(Mutex::new(comm_rx));

    let pool = ThreadPool::new(n);
    for i in 0..n {
        let b_local = b.clone();
        let req_tx_local = req_tx.clone();
        let comm_rx_am_local = comm_rx_am.clone();
        pool.execute(move || thread_function(i, b_local, req_tx_local, comm_rx_am_local));
    }
    // begin ingesting stdin
    let mut command_source = stdin().lines().filter_map(|l| l.ok());
    b.wait();
    loop {
        match req_rx.recv() {
            Ok(signal) => {
                println!("signal: {signal:?}");
                match signal {
                    Signal::NextCommand => {
                        let next_command = command_source.next();
                        println!("next_command: {next_command:?}");
                        comm_tx.send(next_command).unwrap();
                    }
                    Signal::EndThread => {
                        if pool.active_count() == 0 {
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                panic!("Error: {e}");
            }
        }
    }
    pool.join();
}
