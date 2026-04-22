use nix::errno::Errno;
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;

use crate::service::RestartPolicy;

pub fn should_restart(status: WaitStatus, policy: RestartPolicy) -> bool {
    match policy {
        RestartPolicy::Always => true,
        RestartPolicy::Never => false,
        RestartPolicy::OnFailure => match status {
            WaitStatus::Exited(_, code) => code != 0,
            WaitStatus::Signaled(_, _, _) => true,
            _ => false,
        },
    }
}

pub fn reap_children() -> Result<Option<(Pid, WaitStatus)>, nix::Error> {
    match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
        Ok(WaitStatus::StillAlive) => Ok(None),
        Ok(status @ WaitStatus::Exited(pid, _))
        | Ok(status @ WaitStatus::Signaled(pid, _, _))
        | Ok(status @ WaitStatus::Stopped(pid, _))
        | Ok(status @ WaitStatus::Continued(pid))
        | Ok(status @ WaitStatus::PtraceEvent(pid, _, _))
        | Ok(status @ WaitStatus::PtraceSyscall(pid)) => Ok(Some((pid, status))),
        Err(Errno::ECHILD) => Ok(None),
        Err(err) => Err(err),
    }
}
