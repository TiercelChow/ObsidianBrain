//! Daemon management — start/stop/status via PID file and Unix signals (macOS).

use std::fs;
use std::io;
use std::os::fd::AsRawFd;

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

/// Check if a process with the given PID is running (`kill(pid, 0)`).
pub fn is_process_running(pid: i32) -> bool {
    // SAFETY: kill(pid, 0) is safe — it doesn't send a signal, just checks existence.
    let result = unsafe { libc::kill(pid, 0) };
    result == 0
}

/// Check if the daemon is running (PID file exists and process is alive).
pub fn is_running() -> bool {
    match read_pid() {
        Some(pid) => is_process_running(pid),
        None => false,
    }
}

/// Fork a child process, detach from the terminal (setsid), redirect
/// stdin/stdout/stderr to the log file, and write the PID file.
///
/// Returns `Ok(child_pid)` in the parent, or runs the server in the child.
pub fn daemonize() -> io::Result<i32> {
    // Open the log file for the child's stdout/stderr.
    let log_path = paths::log_file();
    let _log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    // Fork.
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }
    if pid > 0 {
        // Parent — child PID is `pid`.
        return Ok(pid);
    }

    // Child — create a new session (detach from terminal).
    unsafe { libc::setsid() };

    // Redirect stdin (from /dev/null), stdout and stderr (to log file).
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

    // Write PID file.
    let _ = write_pid();

    Ok(0) // We're the child — caller checks for 0 to continue.
}

/// Send SIGTERM to the running daemon.
pub fn stop() -> io::Result<()> {
    let pid =
        read_pid().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PID file not found"))?;

    if !is_process_running(pid) {
        remove_pid();
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Process not running",
        ));
    }

    // Send SIGTERM.
    let result = unsafe { libc::kill(pid, libc::SIGTERM) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }

    // Wait for the process to exit (poll up to 5 seconds).
    for _ in 0..50 {
        if !is_process_running(pid) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Force kill if still running.
    if is_process_running(pid) {
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }

    remove_pid();
    Ok(())
}
