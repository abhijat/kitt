use anyhow::{bail, Result};
use nix::sys::signal::Signal;
use nix::sys::wait::WaitStatus;
use nix::sys::{ptrace, signal, wait};
use nix::unistd;
use nix::unistd::{ForkResult, Pid};
use std::ffi::CString;
use std::fmt::{Display, Formatter};

#[derive(Copy, Clone, Eq, PartialEq)]
enum State {
    Stopped,
    Running,
    Exited,
    Terminated,
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
            State::Running => unreachable!(),
        }
    }
}

pub struct Process {
    pid: Pid,
    state: State,
    terminate_on_end: bool,
}

impl Process {
    fn new(pid: Pid, terminate_on_end: bool) -> Self {
        Self {
            pid,
            state: State::Stopped,
            terminate_on_end,
        }
    }

    pub fn launch(path: &str) -> Result<Self> {
        match unsafe { unistd::fork()? } {
            ForkResult::Parent { child } => {
                let mut proc = Process::new(child, true);
                proc.wait_on_signal()?;
                Ok(proc)
            }
            ForkResult::Child => {
                // In the child process. Exec the program now, but first set the process
                // as traceable with PTRACE_TRACEME
                ptrace::traceme()?;
                let program = CString::new(path)?;
                unistd::execvp(&program, &[&program])?;
                // We never reach here because after the above execv succeeds,
                // this code is replaced with program in the process.
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
        let mut proc = Process::new(pid, false);
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
        if self.pid.as_raw() != 0 {
            if self.state == State::Running {
                // If the tracee is running, before detach we must stop it.
                signal::kill(self.pid, Signal::SIGSTOP).expect("failed to send sigkill");
                wait::waitpid(self.pid, None).expect("failed to wait for pid after STOP");
            }

            // detach and continue tracee
            ptrace::detach(self.pid, None).expect("failed to detach from pid");
            signal::kill(self.pid, Signal::SIGCONT).expect("failed to continue pid");

            if self.terminate_on_end {
                signal::kill(self.pid, Signal::SIGKILL).expect("failed to kill pid");
                wait::waitpid(self.pid, None).expect("failed to wait for pid after kill");
            }
        }
    }
}
