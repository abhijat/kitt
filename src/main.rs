use crate::process::{DebugProcess, Process};
use anyhow::Result;
use nix::unistd::Pid;
use rustyline::error::ReadlineError;
use rustyline::history::History;
use rustyline::DefaultEditor;
use std::env;

mod process;
mod reginfo;

fn attach(args: Vec<String>) -> Result<Process> {
    if args.len() == 2 && args[0] == "-p" {
        let pid = args[1].parse()?;
        let pid = Pid::from_raw(pid);
        let process = Process::attach(pid)?;
        Ok(process)
    } else {
        let program_path = &args[0];
        let process = Process::launch(program_path, DebugProcess::YES)?;
        Ok(process)
    }
}

fn handle_command(process: &mut Process, line: &str) -> Result<()> {
    let tokens: Vec<_> = line.split_ascii_whitespace().collect();
    let command = tokens[0];

    if "continue".starts_with(command) {
        process.resume()?;
        let reason = process.wait_on_signal()?;
        println!("process id {} {}", process.pid, reason);
    }

    Ok(())
}

fn handle_command_and_report_errors(process: &mut Process, command: &str) {
    if let Err(err) = handle_command(process, command) {
        println!("{err}");
    }
}

const HISTORY_PATH: &str = ".kitt_hist";

fn repl(process: &mut Process) -> Result<()> {
    let mut editor = DefaultEditor::new()?;
    _ = editor.load_history(HISTORY_PATH);

    loop {
        let line = editor.readline("kitt> ");
        match line {
            Ok(line) if line == "" => {
                let history = editor.history();
                if !history.is_empty() {
                    let last_cmd = &history[history.len() - 1];
                    handle_command_and_report_errors(process, last_cmd);
                }
            }
            Ok(line) => {
                editor.add_history_entry(&line)?;
                handle_command_and_report_errors(process, &line);
            }
            Err(ReadlineError::Interrupted) => {
                println!("Ctrl-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("Ctrl-D");
                break;
            }
            Err(err) => {
                println!("unexpected readline error: {err}");
                break;
            }
        }
    }

    // TODO save periodically? So if we crash we still have the history
    _ = editor
        .save_history(HISTORY_PATH)
        .map_err(|err| eprintln!("failed to save editor history: {err}"));
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    if args.len() == 1 {
        println!("no arguments given");
        std::process::exit(-1);
    }

    let mut process = attach(args.into_iter().skip(1).collect())?;
    if let Err(err) = repl(&mut process) {
        println!("{err}");
    }

    Ok(())
}
