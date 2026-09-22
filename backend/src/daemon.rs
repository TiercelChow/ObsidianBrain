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

/// 命令名是否属于 ObsidianBrain：安装名为 obsidian-brain，
/// 测试二进制名会把连字符换成下划线，统一归一化后再匹配。
fn is_brain_command(command: &str) -> bool {
    command.replace('_', "-").contains("obsidian-brain")
}

/// 端口上属于 ObsidianBrain 的监听进程 PID（用于 PID 文件丢失时的 stop 回退）。
fn brain_listener_pids(port: u16) -> Vec<i32> {
    port_listeners(port)
        .into_iter()
        .filter(|(_, command)| is_brain_command(command))
        .map(|(pid, _)| pid)
        .collect()
}

/// stop 的目标进程：PID 文件指向存活进程时优先信任它；
/// 文件缺失或指向死进程时回退到端口发现（只匹配 ObsidianBrain 自己的进程）。
fn stop_targets(pid: Option<i32>, port: u16) -> Vec<i32> {
    match pid.filter(|pid| is_process_running(*pid)) {
        Some(pid) => vec![pid],
        None => brain_listener_pids(port),
    }
}

/// 端口被占用时的冲突描述；端口空闲返回 None。start 在 daemonize 前用它预检。
pub fn port_conflict_message(port: u16) -> Option<String> {
    let occupants = port_listeners(port);
    if occupants.is_empty() {
        return None;
    }
    let detail = occupants
        .iter()
        .map(|(pid, command)| format!("PID {pid}（{command}）"))
        .collect::<Vec<_>>()
        .join("、");
    Some(format!(
        "端口 {port} 已被 {detail} 占用；请先执行 obsidian-brain stop 或 kill 上述进程"
    ))
}

/// 仅当 PID 文件记录的是当前进程时移除它，避免误删其他实例的 PID 文件。
pub fn remove_own_pid() {
    if read_pid() == Some(std::process::id() as i32) {
        remove_pid();
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

    /// 列出端口上的监听进程（PID + 命令名）。lsof 不可用或端口空闲时返回空。
    /// 只报告事实，不做命令名过滤——调用方决定是回退 stop 还是提示端口冲突。
    pub fn port_listeners(port: u16) -> Vec<(i32, String)> {
        let output = std::process::Command::new("lsof")
            .args(["-t", "-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN"])
            .output();
        let Ok(output) = output else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }
        String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .filter_map(|value| value.parse::<i32>().ok())
            .map(|pid| (pid, process_command(pid).unwrap_or_default()))
            .collect()
    }

    fn process_command(pid: i32) -> Option<String> {
        let output = std::process::Command::new("ps")
            .arg("-p")
            .arg(pid.to_string())
            .arg("-o")
            .arg("comm=")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let command = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if command.is_empty() {
            None
        } else {
            Some(command)
        }
    }

    /// 先 SIGTERM，等待最多 5 秒，仍存活则 SIGKILL。
    pub fn terminate_process(pid: i32) -> io::Result<()> {
        let result = unsafe { libc::kill(pid, libc::SIGTERM) };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        for _ in 0..50 {
            if !is_process_running(pid) {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        unsafe { libc::kill(pid, libc::SIGKILL) };
        Ok(())
    }

    pub fn stop() -> io::Result<()> {
        let port = crate::config::AppConfig::load()
            .unwrap_or_default()
            .server
            .port;
        let targets = stop_targets(read_pid(), port);
        if targets.is_empty() {
            remove_pid();
            // PID 文件失联但端口上有非 ObsidianBrain 进程时，给出可诊断的报错
            if let Some(message) = port_conflict_message(port) {
                return Err(io::Error::other(message));
            }
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "PID file not found",
            ));
        }
        for target in targets {
            terminate_process(target)?;
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

    /// Windows 暂不支持端口发现（stop 回退与 start 预检将自动跳过）。
    pub fn port_listeners(_port: u16) -> Vec<(i32, String)> {
        Vec::new()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_brain_command_accepts_hyphen_and_underscore_binary_names() {
        assert!(is_brain_command("obsidian-brain"));
        assert!(is_brain_command("obsidian_brain-63508b9f40434a21"));
        assert!(!is_brain_command("nginx"));
        assert!(!is_brain_command("python3"));
        assert!(!is_brain_command(""));
    }

    #[cfg(unix)]
    fn lsof_available() -> bool {
        std::process::Command::new("lsof")
            .arg("-v")
            .output()
            .is_ok()
    }

    #[cfg(unix)]
    fn ephemeral_listener() -> (std::net::TcpListener, u16) {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("ephemeral bind");
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    #[cfg(unix)]
    #[test]
    fn test_port_listeners_finds_own_ephemeral_listener() {
        if !lsof_available() {
            return;
        }
        let (_listener, port) = ephemeral_listener();
        let found = port_listeners(port);
        let own = std::process::id() as i32;
        assert!(
            found
                .iter()
                .any(|(pid, command)| *pid == own && is_brain_command(command)),
            "expected own pid {own} among {found:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_port_listeners_returns_empty_for_unused_port() {
        if !lsof_available() {
            return;
        }
        let (_listener, port) = ephemeral_listener();
        drop(_listener);
        assert!(port_listeners(port).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn test_stop_targets_uses_live_pid_file_entry_when_present() {
        let own = std::process::id() as i32;
        assert_eq!(stop_targets(Some(own), 1), vec![own]);
    }

    #[cfg(unix)]
    #[test]
    fn test_stop_targets_falls_back_to_port_listeners_when_pid_missing_or_dead() {
        if !lsof_available() {
            return;
        }
        let (listener, port) = ephemeral_listener();
        let own = std::process::id() as i32;
        assert!(stop_targets(None, port).contains(&own));
        assert!(stop_targets(Some(999_999), port).contains(&own));
        drop(listener);
        assert!(stop_targets(None, port).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn test_port_conflict_message_names_occupier_or_reports_none() {
        if !lsof_available() {
            return;
        }
        let (listener, port) = ephemeral_listener();
        let message = port_conflict_message(port).expect("conflict expected");
        assert!(message.contains(&port.to_string()));
        assert!(message.contains("PID"));
        drop(listener);
        assert!(port_conflict_message(port).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn test_terminate_process_stops_spawned_child() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep");
        terminate_process(child.id() as i32).expect("terminate child");
        let status = child.wait().expect("reap child");
        assert!(!status.success());
    }
}
