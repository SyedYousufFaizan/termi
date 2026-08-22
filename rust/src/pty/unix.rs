//! POSIX PTY (Linux and Android).
//!
//! `portable-pty` 0.8 pulls in the `termios` crate via `serial`/`serial-unix`.
//! That crate only has `cfg(target_os = "linux"|"macos"|"freebsd"|"openbsd")`,
//! so it does not compile for `target_os = "android"` even though Android's
//! bionic libc has the same POSIX PTY syscalls. This module talks to those
//! syscalls through `libc` directly so host tests and the Android NDK build
//! share one implementation.

use crate::utils::error::{PtyError, PtyResult};
use std::ffi::CStr;
use std::fs::File;
use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};

/// PTY window size, matching the classic `winsize` layout.
#[derive(Debug, Clone, Copy)]
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

impl Default for PtySize {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

/// Master side of a PTY pair. Used for ioctl (resize); I/O goes through
/// cloned [`File`] handles so reader and writer can be used independently.
pub struct PtyMaster {
    file: File,
}

impl PtyMaster {
    pub fn resize(&self, size: PtySize) -> PtyResult<()> {
        set_winsize(self.file.as_raw_fd(), size)
    }

    pub fn try_clone_reader(&self) -> PtyResult<File> {
        self.file
            .try_clone()
            .map_err(|e| PtyError::SpawnFailed(format!("Failed to clone PTY reader: {e}")))
    }

    pub fn try_clone_writer(&self) -> PtyResult<File> {
        self.file
            .try_clone()
            .map_err(|e| PtyError::SpawnFailed(format!("Failed to clone PTY writer: {e}")))
    }
}

/// Open a PTY pair, spawn `program` on the slave, and return the master
/// plus the child handle. The slave fd is closed in the parent after spawn.
pub fn spawn_on_pty(
    program: &str,
    size: PtySize,
    cwd: Option<&str>,
) -> PtyResult<(PtyMaster, Child)> {
    let (master, slave) = open_pty_pair(size)?;
    let slave_fd = slave.as_raw_fd();

    let mut cmd = Command::new(program);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("LANG", "en_US.UTF-8");
    // stdin/stdout/stderr are replaced in pre_exec via dup2; Stdio::null
    // avoids inheriting the parent's terminals in the meantime.
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    // SAFETY: `slave_fd` is a live fd owned by `slave` until we drop it
    // after `spawn`. The child `pre_exec` hook runs in the forked process
    // before exec, where duplicating/closing that fd is the standard POSIX
    // login-tty sequence. We do not use `slave` in the parent after spawn.
    unsafe {
        cmd.pre_exec(move || setup_child_tty(slave_fd));
    }

    let child = cmd
        .spawn()
        .map_err(|e| PtyError::SpawnFailed(format!("Failed to spawn {program}: {e}")))?;

    // Parent no longer needs the slave; closing it means the child gets
    // EOF on the slave when the master is closed.
    drop(slave);

    Ok((PtyMaster { file: master }, child))
}

fn open_pty_pair(size: PtySize) -> PtyResult<(File, File)> {
    let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC) };
    if master_fd < 0 {
        return Err(PtyError::SpawnFailed(format!(
            "posix_openpt failed: {}",
            io::Error::last_os_error()
        )));
    }

    // SAFETY: posix_openpt returned a freshly opened fd we now own.
    let master = unsafe { File::from_raw_fd(master_fd) };

    if unsafe { libc::grantpt(master_fd) } != 0 {
        return Err(PtyError::SpawnFailed(format!(
            "grantpt failed: {}",
            io::Error::last_os_error()
        )));
    }
    if unsafe { libc::unlockpt(master_fd) } != 0 {
        return Err(PtyError::SpawnFailed(format!(
            "unlockpt failed: {}",
            io::Error::last_os_error()
        )));
    }

    let slave_path = ptsname(master_fd)?;
    let slave = File::options()
        .read(true)
        .write(true)
        .open(&slave_path)
        .map_err(|e| PtyError::SpawnFailed(format!("Failed to open slave {slave_path}: {e}")))?;

    set_winsize(master_fd, size)?;
    Ok((master, slave))
}

fn ptsname(master_fd: RawFd) -> PtyResult<String> {
    let mut buf = [0u8; 128];
    let rc =
        unsafe { libc::ptsname_r(master_fd, buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if rc != 0 {
        return Err(PtyError::SpawnFailed(format!(
            "ptsname_r failed: {}",
            io::Error::last_os_error()
        )));
    }
    let name = unsafe { CStr::from_ptr(buf.as_ptr() as *const libc::c_char) };
    name.to_str()
        .map(|s| s.to_string())
        .map_err(|_| PtyError::SpawnFailed("ptsname_r returned non-UTF8 path".into()))
}

fn set_winsize(fd: RawFd, size: PtySize) -> PtyResult<()> {
    let ws = libc::winsize {
        ws_row: size.rows,
        ws_col: size.cols,
        ws_xpixel: size.pixel_width,
        ws_ypixel: size.pixel_height,
    };
    let rc = unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &ws) };
    if rc != 0 {
        return Err(PtyError::ResizeFailed(format!(
            "TIOCSWINSZ failed: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(())
}

/// POSIX login-tty sequence, run in the child between fork and exec.
fn setup_child_tty(slave_fd: RawFd) -> io::Result<()> {
    if unsafe { libc::setsid() } < 0 {
        return Err(io::Error::last_os_error());
    }
    // TIOCSCTTY can fail if we already have a controlling tty; not fatal
    // for a shell spawn as long as stdio is the slave.
    let _ = unsafe { libc::ioctl(slave_fd, libc::TIOCSCTTY, 0) };
    if unsafe { libc::dup2(slave_fd, 0) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::dup2(slave_fd, 1) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::dup2(slave_fd, 2) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if slave_fd > 2 {
        unsafe { libc::close(slave_fd) };
    }
    Ok(())
}

/// Map a child exit into the `i32` the rest of the crate already uses.
pub fn child_exit_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(|s| 128 + s).unwrap_or(1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_open_pty_pair() {
        let (master, slave) = open_pty_pair(PtySize::default()).unwrap();
        assert!(master.as_raw_fd() >= 0);
        assert!(slave.as_raw_fd() >= 0);
        set_winsize(
            master.as_raw_fd(),
            PtySize {
                rows: 40,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            },
        )
        .unwrap();
    }
}
