use nix::errno::Errno;
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;
use std::time::{Duration, Instant};

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

pub fn next_restart_delay(
    now: Instant,
    last_start: Instant,
    restart_count: &mut u32,
    respawn_delay_secs: u64,
    respawn_window_secs: u64,
    respawn_max: u32,
) -> Option<Duration> {
    let window = Duration::from_secs(respawn_window_secs);
    if now.duration_since(last_start) > window {
        *restart_count = 0;
    }

    if respawn_max > 0 && *restart_count >= respawn_max {
        return None;
    }

    let exponent = (*restart_count).min(6);
    let multiplier = 1u64 << exponent;
    *restart_count += 1;

    Some(Duration::from_secs(
        respawn_delay_secs.saturating_mul(multiplier),
    ))
}

#[cfg(test)]
mod tests {
    use super::next_restart_delay;
    use std::time::{Duration, Instant};

    #[test]
    fn respawn_delay_grows_exponentially_and_caps() {
        let base = Instant::now();
        let mut restart_count = 0;

        let delay1 = next_restart_delay(base, base, &mut restart_count, 2, 60, 0).unwrap();
        let delay2 = next_restart_delay(base, base, &mut restart_count, 2, 60, 0).unwrap();
        let delay3 = next_restart_delay(base, base, &mut restart_count, 2, 60, 0).unwrap();

        assert_eq!(delay1, Duration::from_secs(2));
        assert_eq!(delay2, Duration::from_secs(4));
        assert_eq!(delay3, Duration::from_secs(8));
    }

    #[test]
    fn respawn_count_resets_after_window() {
        let now = Instant::now();
        let mut restart_count = 4;

        let delay = next_restart_delay(
            now,
            now - Duration::from_secs(61),
            &mut restart_count,
            3,
            60,
            0,
        )
        .unwrap();

        assert_eq!(delay, Duration::from_secs(3));
        assert_eq!(restart_count, 1);
    }

    #[test]
    fn respawn_max_stops_restarts() {
        let now = Instant::now();
        let mut restart_count = 2;

        let delay = next_restart_delay(now, now, &mut restart_count, 1, 60, 2);

        assert!(delay.is_none());
    }
}
