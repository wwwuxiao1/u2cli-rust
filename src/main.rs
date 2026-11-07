mod cli;
mod device;
mod exec;
mod help;
mod hierarchy;
mod json;
mod output;
mod server;

use std::env;
use std::process::ExitCode;

/// 程序入口：读取命令行参数并交给命令分发。
fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    ExitCode::from(run(&args))
}

/// 执行一次 u2cli 调用，返回进程退出码。
pub fn run(args: &[String]) -> u8 {
    let invocation = match cli::parse(args) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Error: {error}");
            return 2;
        }
    };
    if invocation.show_help {
        print!("{}", help::render(&invocation));
        return 0;
    }
    match exec::dispatch(&invocation) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(error) => {
            eprintln!("error: {error}");
            1
        }
    }
}
