use std::env::{var, VarError};
use std::io::stdin;
use std::process::Command;
use std::sync::{Arc, Barrier, LockResult, Mutex};
use threadpool::ThreadPool;

#[derive(Debug)]
enum EnvThreadsResult {
    Value(usize),
    Unset,
    Err,
}

fn get_threads() -> EnvThreadsResult {
    match var("PARALLEL_THREADS") {
        Ok(string) => match string.parse::<usize>() {
            Ok(v) => EnvThreadsResult::Value(v),
            Err(_r) => EnvThreadsResult::Err,
        },
        Err(r) => match r {
            VarError::NotPresent => EnvThreadsResult::Unset,
            VarError::NotUnicode(_) => EnvThreadsResult::Err,
        },
    }
}

fn main() {
    let t = get_threads();
    if let EnvThreadsResult::Value(n) = t {
        let pool = ThreadPool::new(n);
        let b = Arc::new(Barrier::new(n + 1));
        let command_list: Vec<String> = stdin()
            .lines()
            .filter_map(|l| l.ok())
            .collect::<Vec<String>>();
        println!("{command_list:?}");
        let command_list_m = Mutex::new(command_list);
        let command_list_am = Arc::new(command_list_m);
        for i in 0..n {
            println!("creating thread: {i}");
            let b_local = b.clone();
            let command_list_local = command_list_am.clone();
            pool.execute(move || {
                b_local.wait();
                loop {
                    let command = match command_list_local.lock() {
                        LockResult::Ok(mut v) => {
                            let next_command = v.pop();
                            match next_command {
                                None => {
                                    break;
                                }
                                Some(c) => c,
                            }
                        }
                        LockResult::Err(_) => {
                            break;
                        }
                    };
                    println!("thread {i}) {command}");
                    let r = Command::new("sh")
                        .arg("-c")
                        .arg(format!("{command}"))
                        .env("THREAD_INDEX", format!("{i}"))
                        .status();
                    if let Err(e) = r {
                        println!("Error on process: {e}");
                        break;
                    }
                }
            });
        }
        b.wait();
        pool.join();
    }
}
