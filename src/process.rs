use anyhow::{bail, Result};
use nix::sys::signal::Signal;
use nix::sys::wait::WaitStatus;
use nix::sys::{ptrace, signal, wait};
use nix::unistd;
use nix::unistd::{ForkResult, Pid};
use std::cmp::PartialEq;
use std::ffi::CString;
use std::fmt::{Display, Formatter};
use std::io::{pipe, Read};
use std::io::{PipeReader, Write};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ProcessState {
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
    process_state: ProcessState,
    stop_cause: StopCause,
}

impl StopReason {
    pub fn new(wait_status: WaitStatus) -> Self {
        match wait_status {
            WaitStatus::Exited(_, code) => Self {
                process_state: ProcessState::Exited,
                stop_cause: StopCause::Code(code),
            },
            WaitStatus::Signaled(_, signal, _) => Self {
                process_state: ProcessState::Terminated,
                stop_cause: StopCause::Signal(signal),
            },
            WaitStatus::Stopped(_, signal) => Self {
                process_state: ProcessState::Stopped,
                stop_cause: StopCause::Signal(signal),
            },
            unexpected => panic!("unexpected wait status {unexpected:?}"),
        }
    }
}

impl Display for StopReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.process_state {
            ProcessState::Stopped => write!(f, "stopped with cause {}", self.stop_cause),
            ProcessState::Exited => write!(f, "terminated with signal {}", self.stop_cause),
            ProcessState::Terminated => write!(f, "stopped with signal {}", self.stop_cause),
            ProcessState::Running | ProcessState::FailedToLaunch => unreachable!(),
        }
    }
}

#[derive(PartialEq)]
enum TerminateOnEnd {
    YES,
    NO,
}

#[derive(PartialEq, Copy, Clone)]
pub enum DebugProcess {
    YES,
    NO,
}

#[derive(PartialEq, Debug)]
enum IsAttached {
    YES,
    NO,
}

impl From<DebugProcess> for IsAttached {
    fn from(value: DebugProcess) -> Self {
        match value {
            DebugProcess::YES => IsAttached::YES,
            DebugProcess::NO => IsAttached::NO,
        }
    }
}

pub struct Process {
    pub(crate) pid: Pid,
    state: ProcessState,
    terminate_on_end: TerminateOnEnd,
    is_attached: IsAttached,
}

fn read_from_pipe(mut r: PipeReader) -> Result<String> {
    let mut buf = String::new();
    r.read_to_string(&mut buf)?;
    Ok(buf)
}

impl Process {
    fn new(pid: Pid, terminate_on_end: TerminateOnEnd, is_attached: IsAttached) -> Self {
        Self {
            pid,
            state: ProcessState::Stopped,
            terminate_on_end,
            is_attached,
        }
    }

    pub fn launch(path: &str, debug_process: DebugProcess) -> Result<Self> {
        // O_CLOEXEC is set by `pipe_inner`
        let (reader, mut writer) = pipe()?;
        match unsafe { unistd::fork()? } {
            ForkResult::Parent { child } => {
                let mut proc = Process::new(child, TerminateOnEnd::YES, debug_process.into());

                drop(writer);

                let msg = read_from_pipe(reader)?;
                if !msg.is_empty() {
                    proc.state = ProcessState::FailedToLaunch;
                    bail!("child failed to launch: {msg}");
                }

                if debug_process == DebugProcess::YES {
                    proc.wait_on_signal()?;
                }
                Ok(proc)
            }
            ForkResult::Child => {
                drop(reader);
                // In the child process. Exec the program now, but first set the process
                // as traceable with PTRACE_TRACEME
                if debug_process == DebugProcess::YES {
                    ptrace::traceme()?;
                }

                let program = CString::new(path)?;
                let result = unistd::execvp(&program, &[&program]);
                // If we reach here, it is because execvp failed. The result is guaranteed
                // to contain an error.
                write!(writer, "{}", result.err().unwrap())?;
                // No one will receive this error
                bail!("failed to launch");
            }
        }
    }

    pub fn attach(pid: Pid) -> Result<Self> {
        // Calls PTRACE_ATTACH
        ptrace::attach(pid)?;
        let mut proc = Process::new(pid, TerminateOnEnd::NO, IsAttached::YES);
        proc.wait_on_signal()?;
        Ok(proc)
    }

    // Resume the traced process with PTRACE_CONT
    pub fn resume(&mut self) -> Result<()> {
        ptrace::cont(self.pid, None)?;
        self.state = ProcessState::Running;
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
}

impl Drop for Process {
    // When the process wrapper is destroyed, we want to stop a running process,
    // detach from the traced process using PTRACE_DETACH, and finally kill the
    // process if configured (via terminate_on_end).
    fn drop(&mut self) {
        if self.pid.as_raw() == 0 {
            return;
        }

        if self.state == ProcessState::Exited
            || self.state == ProcessState::Terminated
            || self.state == ProcessState::FailedToLaunch
        {
            return;
        }

        if self.is_attached == IsAttached::YES {
            if self.state == ProcessState::Running {
                // If the tracee is running, before detach we must stop it.
                signal::kill(self.pid, Signal::SIGSTOP).expect("failed to send sigkill");
                self.wait_on_signal()
                    .expect("failed while waiting for state change after SIGSTOP");
            }

            // detach and continue tracee
            ptrace::detach(self.pid, None).expect("failed to detach from pid");
            signal::kill(self.pid, Signal::SIGCONT).expect("failed to continue pid");
        }

        if self.terminate_on_end == TerminateOnEnd::YES {
            signal::kill(self.pid, Signal::SIGKILL).expect("failed to kill pid");
            wait::waitpid(self.pid, None).expect("failed to wait for pid after kill");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::process::{DebugProcess, Process, ProcessState};
    use anyhow::Result;
    use nix::sys::signal;
    use nix::unistd::Pid;
    use std::fs;

    fn process_exists(pid: Pid) -> bool {
        signal::kill(pid, None).is_ok()
    }

    fn process_state(pid: Pid) -> Result<char> {
        let data = fs::read_to_string(format!("/proc/{}/stat", pid.as_raw()))?;
        let last_paren = data.rfind(')').unwrap();
        let c = data.chars().nth(last_paren + 2);
        Ok(c.unwrap())
    }

    #[test]
    fn process_exists_when_launched() {
        let process = Process::launch("yes", DebugProcess::YES);
        assert!(process.is_ok());

        let process = process.unwrap();
        assert!(process_exists(process.pid));
    }

    #[test]
    fn failure_when_launching_imaginary_program() {
        let process = Process::launch("this-program-198533233-never-was", DebugProcess::YES);
        assert!(process.is_err_and(|err| err.to_string().contains("child failed to launch")));
    }

    #[test]
    fn safe_to_drop_exited_process() {
        let process = Process::launch("ls", DebugProcess::YES);
        assert!(process.is_ok());

        let mut process = process.unwrap();

        process.resume().unwrap();
        let wait_res = process.wait_on_signal().unwrap();

        assert!(matches!(wait_res.process_state, ProcessState::Exited));

        assert!(!process_exists(process.pid));
        drop(process);
    }

    #[test]
    fn attach_success() {
        let forever = Process::launch("target/debug/run-forever", DebugProcess::NO);
        assert!(forever.is_ok());

        let forever = forever.unwrap();

        let attached = Process::attach(forever.pid);
        assert!(attached.is_ok());
        assert!(matches!(process_state(attached.unwrap().pid), Ok('t')));
    }

    #[test]
    fn attach_to_invalid_pid() {
        let attached = Process::attach(Pid::from_raw(0));
        assert!(attached.is_err_and(|err| err.to_string().contains("ESRCH: No such process")));
    }

    #[test]
    fn resumed_process_is_in_appropriate_state() {
        for debugging in [DebugProcess::YES, DebugProcess::NO] {
            let p = Process::launch("target/debug/run-forever", debugging);
            assert!(p.is_ok());

            let p = p.unwrap();
            let mut p = if debugging == DebugProcess::YES {
                p
            } else {
                let p = Process::attach(p.pid);
                assert!(p.is_ok());
                p.unwrap()
            };

            p.resume().unwrap();
            let status = process_state(p.pid);

            assert!(matches!(status, Ok('R')) || matches!(status, Ok('S')));
        }
    }

    #[test]
    fn finished_program_cannot_resume() {
        let p = Process::launch("ls", DebugProcess::YES);
        assert!(p.is_ok());
        let mut p = p.unwrap();
        assert!(p.resume().is_ok());
        assert!(p.wait_on_signal().is_ok());
        assert!(
            p.resume()
                .is_err_and(|err| err.to_string().contains("ESRCH"))
        );
    }
}
