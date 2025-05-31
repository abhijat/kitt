use crate::process::Process;
use anyhow::Result;
use nix::unistd::Pid;
use rustyline::error::ReadlineError;
use rustyline::history::History;
use rustyline::DefaultEditor;
use std::env;

mod process;

fn attach(args: Vec<String>) -> Result<Process> {
    if args.len() == 2 && args[0] == "-p" {
        let pid = args[1].parse()?;
        let pid = Pid::from_raw(pid);
        let process = Process::attach(pid)?;
        Ok(process)
    } else {
        let program_path = &args[0];
        let process = Process::launch(program_path)?;
        Ok(process)
    }
}

fn handle_command(process: &mut Process, line: &str) -> Result<()> {
    let tokens: Vec<_> = line.split_ascii_whitespace().collect();
    let command = tokens[0];

    if "continue".starts_with(command) {
        process.resume()?;
        let reason = process.wait_on_signal()?;
        println!("process id {} {}", process.process_id(), reason);
    }

    Ok(())
}

fn repl(process: &mut Process) -> Result<()> {
    let mut editor = DefaultEditor::new()?;
    let _ = editor.load_history("history.txt");

    loop {
        let line = editor.readline("kitt> ");
        match line {
            Ok(line) if line == "" => {
                let history = editor.history();
                if !history.is_empty() {
                    let last = history.len() - 1;
                    let last_cmd = &history[last];
                    handle_command(process, last_cmd)?
                }
            }
            Ok(line) => {
                editor.add_history_entry(&line)?;
                handle_command(process, &line)?;
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
