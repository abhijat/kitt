use anyhow::{Result, bail};
use nix::sys::signal::Signal;
use nix::sys::wait::WaitStatus;
use nix::sys::{ptrace, signal, wait};
use nix::unistd;
use nix::unistd::{ForkResult, Pid};
use std::cmp::PartialEq;
use std::ffi::CString;
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::io::{Read, pipe};

#[derive(Copy, Clone, Eq, PartialEq)]
enum State {
    Stopped,
    Running,
    Exited,
    Terminated,
    FailedToLaunch,
}

enum StopCause {
    Signal(Signal),
    Code(i32),
}

impl Display for StopCause {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StopCause::Signal(signal) => write!(f, "signal {signal}"),
            StopCause::Code(code) => write!(f, "exit code {code}"),
        }
    }
}

pub struct StopReason {
    process_state: State,
    stop_cause: StopCause,
}

impl StopReason {
    pub fn new(wait_status: WaitStatus) -> Self {
        match wait_status {
            WaitStatus::Exited(_, code) => Self {
                process_state: State::Exited,
                stop_cause: StopCause::Code(code),
            },
            WaitStatus::Signaled(_, signal, _) => Self {
                process_state: State::Terminated,
                stop_cause: StopCause::Signal(signal),
            },
            WaitStatus::Stopped(_, signal) => Self {
                process_state: State::Stopped,
                stop_cause: StopCause::Signal(signal),
            },
            unexpected => panic!("unexpected wait status {unexpected:?}"),
        }
    }
}

impl Display for StopReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.process_state {
            State::Stopped => write!(f, "stopped with cause {}", self.stop_cause),
            State::Exited => write!(f, "terminated with signal {}", self.stop_cause),
            State::Terminated => write!(f, "stopped with signal {}", self.stop_cause),
            State::Running | State::FailedToLaunch => unreachable!(),
        }
    }
}

#[derive(Eq, PartialEq)]
enum TerminateOnEnd {
    YES,
    NO,
}

pub struct Process {
    pid: Pid,
    state: State,
    terminate_on_end: TerminateOnEnd,
}

impl Process {
    fn new(pid: Pid, terminate_on_end: TerminateOnEnd) -> Self {
        Self {
            pid,
            state: State::Stopped,
            terminate_on_end,
        }
    }

    pub fn launch(path: &str) -> Result<Self> {
        // O_CLOEXEC is set by `pipe_inner`
        let (mut reader, mut writer) = pipe()?;
        match unsafe { unistd::fork()? } {
            ForkResult::Parent { child } => {
                let mut proc = Process::new(child, TerminateOnEnd::YES);

                drop(writer);
                let mut buf = String::new();
                let data = reader.read_to_string(&mut buf)?;

                if data != 0 {
                    proc.state = State::FailedToLaunch;
                    bail!("child failed to launch: {buf}");
                }

                proc.wait_on_signal()?;
                Ok(proc)
            }
            ForkResult::Child => {
                drop(reader);
                // In the child process. Exec the program now, but first set the process
                // as traceable with PTRACE_TRACEME
                ptrace::traceme()?;
                let program = CString::new(path)?;
                let result = unistd::execvp(&program, &[&program]);
                // If we reach here, it is because execvp failed. The result is guaranteed
                // to contain an error.
                let Err(err) = result;
                write!(writer, "{err}")?;
                unreachable!()
            }
        }
    }

    pub fn attach(pid: Pid) -> Result<Self> {
        if pid.as_raw() == 0 {
            bail!("Invalid process id: {pid}");
        }

        // Calls PTRACE_ATTACH
        ptrace::attach(pid)?;
        let mut proc = Process::new(pid, TerminateOnEnd::NO);
        proc.wait_on_signal()?;
        Ok(proc)
    }

    // Resume the traced process with PTRACE_CONT
    pub fn resume(&self) -> Result<()> {
        ptrace::cont(self.pid, None)?;
        Ok(())
    }

    // Waits on the pid. waitpid will block until the status of the watched process changes.
    // The return value contains information about what changes were observed.
    pub fn wait_on_signal(&mut self) -> Result<StopReason> {
        let wait_result = wait::waitpid(self.pid, None)?;
        let stop_reason = StopReason::new(wait_result);
        self.state = stop_reason.process_state;
        Ok(stop_reason)
    }

    pub fn process_id(&self) -> Pid {
        self.pid
    }
}

impl Drop for Process {
    // When the process wrapper is destroyed, we want to stop a running process,
    // detach from the traced process using PTRACE_DETACH, and finally kill the
    // process if configured (via terminate_on_end).
    fn drop(&mut self) {
        if self.pid.as_raw() == 0 || self.state == State::FailedToLaunch {
            return;
        }

        if self.state == State::Running {
            // If the tracee is running, before detach we must stop it.
            signal::kill(self.pid, Signal::SIGSTOP).expect("failed to send sigkill");
            self.wait_on_signal()
                .expect("failed while waiting for state change after SIGSTOP");
        }

        if self.state != State::Stopped {
            // The process is terminated or has exited. We can no longer interact with it now.
            return;
        }

        // detach and continue tracee
        ptrace::detach(self.pid, None).expect("failed to detach from pid");
        signal::kill(self.pid, Signal::SIGCONT).expect("failed to continue pid");

        if self.terminate_on_end == TerminateOnEnd::YES {
            signal::kill(self.pid, Signal::SIGKILL).expect("failed to kill pid");
            wait::waitpid(self.pid, None).expect("failed to wait for pid after kill");
        }
    }
}

fn process_exists(pid: Pid) -> bool {
    signal::kill(pid, None).is_ok()
}

#[cfg(test)]
mod tests {
    use crate::process::{Process, process_exists};

    #[test]
    fn process_exists_when_launched() {
        let process = Process::launch("yes");
        assert!(process.is_ok());

        let process = process.unwrap();
        assert!(process_exists(process.process_id()));
    }

    #[test]
    fn failure_when_launching_imaginary_program() {
        let process = Process::launch("this-program-198533233-never-was");
        assert!(process.is_err_and(|err| err.to_string().contains("child failed to launch")));
    }
}
