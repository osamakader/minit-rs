use nix::errno::Errno;
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;
use std::process::Command;
use std::thread;
use std::time::Duration;

struct Service {
    name: &'static str,
    command: &'static [&'static str],
}

fn spawn_service(service: &Service) -> std::io::Result<u32> {
    let mut child = Command::new(service.command[0]);
    child.args(&service.command[1..]);
    let child = child.spawn()?;
    println!(
        "spawned service '{}' with pid {}",
        service.name,
        child.id()
    );
    Ok(child.id())
}

fn reap_children() -> Result<Option<(Pid, WaitStatus)>, nix::Error> {
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = Service {
        name: "example",
        command: &["/bin/sh", "-c", "echo service started; sleep 3; echo service done"],
    };
    let target_pid = Pid::from_raw(spawn_service(&service)? as i32);

    loop {
        match reap_children()? {
            Some((pid, status)) => {
                println!("reaped pid {} with status {:?}", pid, status);
                if pid == target_pid {
                    println!("hardcoded service exited, stopping init loop");
                    break;
                }
            }
            None => {
                thread::sleep(Duration::from_millis(200));
            }
        }
    }

    Ok(())
}
