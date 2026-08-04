//! Daemon management — start/stop/status via PID file.
//!
//! Platform-specific implementations:
//! - Unix (macOS/Linux): fork/setsid/SIGTERM via libc
//! - Windows: spawn detached child + taskkill

use std::fs;
use std::io;

use crate::paths;

/// Read the PID from the PID file. Returns None if the file doesn't exist or is invalid.
pub fn read_pid() -> Option<i32> {
    let content = fs::read_to_string(paths::pid_file()).ok()?;
    content.trim().parse::<i32>().ok()
}

/// Write the current process PID to the PID file.
pub fn write_pid() -> io::Result<()> {
    let pid = std::process::id() as i32;
    fs::write(paths::pid_file(), pid.to_string())
}

/// Remove the PID file.
pub fn remove_pid() {
    let _ = fs::remove_file(paths::pid_file());
}

/// Check if the daemon is running (PID file exists and process is alive).
pub fn is_running() -> bool {
    match read_pid() {
        Some(pid) => is_process_running(pid),
        None => false,
    }
}

// ── Unix implementation ────────────────────────────────────────────────

#[cfg(unix)]
mod platform {
    use super::*;
    use std::os::fd::AsRawFd;

    pub fn is_process_running(pid: i32) -> bool {
        let result = unsafe { libc::kill(pid, 0) };
        result == 0
    }

    pub fn daemonize() -> io::Result<i32> {
        let log_path = paths::log_file();
        let _log_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Err(io::Error::last_os_error());
        }
        if pid > 0 {
            return Ok(pid);
        }

        unsafe { libc::setsid() };

        let dev_null = fs::OpenOptions::new().read(true).open("/dev/null")?;
        let log_file2 = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        unsafe {
            libc::dup2(dev_null.as_raw_fd(), 0);
            libc::dup2(log_file2.as_raw_fd(), 1);
            libc::dup2(log_file2.as_raw_fd(), 2);
        }

        let _ = write_pid();
        Ok(0)
    }

    pub fn stop() -> io::Result<()> {
        let pid = read_pid()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PID file not found"))?;

        if !is_process_running(pid) {
            remove_pid();
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Process not running",
            ));
        }

        let result = unsafe { libc::kill(pid, libc::SIGTERM) };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }

        for _ in 0..50 {
            if !is_process_running(pid) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        if is_process_running(pid) {
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }

        remove_pid();
        Ok(())
    }
}

// ── Windows implementation ─────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use super::*;

    pub fn is_process_running(pid: i32) -> bool {
        // On Windows, use tasklist to check if a PID exists.
        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output();
        match output {
            Ok(o) => String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()),
            Err(_) => false,
        }
    }

    pub fn daemonize() -> io::Result<i32> {
        // Windows: spawn a detached child process running `start --foreground`.
        let exe = std::env::current_exe()?;
        let log_path = paths::log_file();
        let log_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        let child = std::process::Command::new(&exe)
            .arg("start")
            .arg("--foreground")
            .stdout(std::process::Stdio::from(log_file.try_clone()?))
            .stderr(std::process::Stdio::from(log_file))
            .stdin(std::process::Stdio::null())
            .spawn()?;

        let pid = child.id() as i32;

        // Write PID file with the child's PID.
        fs::write(paths::pid_file(), pid.to_string())?;

        Ok(pid)
    }

    pub fn stop() -> io::Result<()> {
        let pid = read_pid()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PID file not found"))?;

        if !is_process_running(pid) {
            remove_pid();
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Process not running",
            ));
        }

        // taskkill /PID {pid} /T /F — kill the process tree.
        let status = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()?;

        if !status.success() {
            return Err(io::Error::new(io::ErrorKind::Other, "taskkill failed"));
        }

        remove_pid();
        Ok(())
    }
}

// ── Public API (delegates to platform module) ──────────────────────────

pub use platform::*;
