//! POSIX PTY (Linux and Android).
//!
//! `portable-pty` 0.8 pulls in the `termios` crate via `serial`/`serial-unix`.
//! That crate only has `cfg(target_os = "linux"|"macos"|"freebsd"|"openbsd")`,
//! so it does not compile for `target_os = "android"` even though Android's
//! bionic libc has the same POSIX PTY syscalls. This module talks to those
//! syscalls through `libc` directly so host tests and the Android NDK build
//! share one implementation.

use crate::utils::error::{PtyError, PtyResult};
use log::{info, warn};
use std::ffi::CStr;
use std::fs::{File, OpenOptions};
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
///
/// On Android, opening `/dev/pts/N` from the parent and `fork`+`setsid` in
/// `pre_exec` both fail on some OEMs. We try a full login-tty spawn first,
/// then fall back to attaching the slave as the child's stdio without a
/// controlling tty so a session can still start.
pub fn spawn_on_pty(
    program: &str,
    size: PtySize,
    cwd: Option<&str>,
) -> PtyResult<(PtyMaster, Child)> {
    let (master, slave) = open_pty_pair(size)?;

    match spawn_login_tty(program, cwd, slave.as_raw_fd()) {
        Ok(child) => {
            drop(slave);
            return Ok((PtyMaster { file: master }, child));
        }
        Err(e) => {
            warn!(
                "login-tty spawn of {program} failed ({e}); retrying with slave stdio (no controlling tty)"
            );
        }
    }

    match spawn_slave_stdio(program, cwd, &slave) {
        Ok(child) => {
            drop(slave);
            Ok((PtyMaster { file: master }, child))
        }
        Err(e) => Err(PtyError::SpawnFailed(format!(
            "Failed to spawn {program}: {e}"
        ))),
    }
}

fn configure_command(cmd: &mut Command, cwd: Option<&str>) {
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("LANG", "en_US.UTF-8");
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
        cmd.env("HOME", dir);
        cmd.env("PWD", dir);
    }
    #[cfg(target_os = "android")]
    {
        cmd.env(
            "PATH",
            "/product/bin:/apex/com.android.runtime/bin:/system_ext/bin:/system/bin:/system/xbin:/vendor/bin",
        );
        cmd.env("ANDROID_DATA", "/data");
        cmd.env("ANDROID_ROOT", "/system");
    }
}

/// Full POSIX login-tty: setsid + TIOCSCTTY + dup2 in the child.
fn spawn_login_tty(program: &str, cwd: Option<&str>, slave_fd: RawFd) -> io::Result<Child> {
    let mut cmd = Command::new(program);
    configure_command(&mut cmd, cwd);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());

    // SAFETY: `slave_fd` stays live in the parent `File` until after spawn.
    // pre_exec runs in the child between fork and exec.
    unsafe {
        cmd.pre_exec(move || setup_child_tty(slave_fd));
    }

    cmd.spawn()
}

/// Attach cloned slave fds as stdin/stdout/stderr. No `pre_exec`, so Android
/// can use `posix_spawn` instead of `fork`. Ctrl+C via the kernel may not
/// work without a controlling tty; the toolbar can still write 0x03.
fn spawn_slave_stdio(program: &str, cwd: Option<&str>, slave: &File) -> io::Result<Child> {
    let mut cmd = Command::new(program);
    configure_command(&mut cmd, cwd);
    cmd.stdin(Stdio::from(slave.try_clone()?));
    cmd.stdout(Stdio::from(slave.try_clone()?));
    cmd.stderr(Stdio::from(slave.try_clone()?));
    cmd.spawn()
}

fn open_pty_pair(size: PtySize) -> PtyResult<(File, File)> {
    let master = open_ptmx()?;
    let master_fd = master.as_raw_fd();

    // bionic's grantpt is a no-op; some OEMs still return an error. Do not
    // abort session create for that.
    if unsafe { libc::grantpt(master_fd) } != 0 {
        warn!(
            "grantpt failed (continuing): {}",
            io::Error::last_os_error()
        );
    }
    if unsafe { libc::unlockpt(master_fd) } != 0 {
        return Err(PtyError::SpawnFailed(format!(
            "unlockpt failed: {}",
            io::Error::last_os_error()
        )));
    }

    let slave = open_slave(master_fd)?;
    if let Err(e) = set_winsize(master_fd, size) {
        warn!("TIOCSWINSZ failed (continuing): {e}");
    }
    Ok((master, slave))
}

/// bionic's `ioctl` request is `c_int`; glibc's is `c_ulong`. Host
/// `cargo check --features android` uses the host libc and will not catch
/// this — always also check `--target aarch64-linux-android`.
#[cfg(target_os = "android")]
type IoctlReq = libc::c_int;
#[cfg(not(target_os = "android"))]
type IoctlReq = libc::c_ulong;

/// asm-generic ioctl numbers (same on Linux and Android).
/// TIOCGPTN is not exported by `libc` for `target_os = "android"`.
const TIOCSCTTY: IoctlReq = 0x540E as IoctlReq;
const TIOCSWINSZ: IoctlReq = 0x5414 as IoctlReq;
/// `_IOR('T', 0x30, unsigned int)` — bit 31 is set, so the Android
/// `c_int` request is negative. Cast from `u32` to avoid
/// `overflowing_literals` on `target_os = "android"`.
const TIOCGPTN: IoctlReq = 0x8004_5430u32 as IoctlReq;
/// `_IO('T', 0x41)` — open slave from master without `/dev/pts`
const TIOCGPTPEER: IoctlReq = 0x5441 as IoctlReq;

fn open_ptmx() -> PtyResult<File> {
    let fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC) };
    if fd >= 0 {
        // SAFETY: posix_openpt returned a freshly opened fd we now own.
        return Ok(unsafe { File::from_raw_fd(fd) });
    }
    let posix_err = io::Error::last_os_error();
    OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/ptmx")
        .map_err(|e| {
            PtyError::SpawnFailed(format!(
                "posix_openpt failed ({posix_err}); open(/dev/ptmx) failed ({e})"
            ))
        })
}

fn open_slave(master_fd: RawFd) -> PtyResult<File> {
    let flags = libc::O_RDWR | libc::O_NOCTTY;
    let peer = unsafe { libc::ioctl(master_fd, TIOCGPTPEER, flags) };
    if peer >= 0 {
        info!("Opened PTY slave via TIOCGPTPEER");
        // SAFETY: ioctl returned a new fd on success.
        return Ok(unsafe { File::from_raw_fd(peer as RawFd) });
    }
    let peer_err = io::Error::last_os_error();

    if let Ok(n) = pty_index(master_fd) {
        let path = format!("/dev/pts/{n}");
        match File::options().read(true).write(true).open(&path) {
            Ok(f) => {
                info!("Opened PTY slave {path}");
                return Ok(f);
            }
            Err(e) => warn!("open({path}) failed: {e}"),
        }
    }

    let path = ptsname(master_fd)?;
    File::options()
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|e| {
            PtyError::SpawnFailed(format!(
                "Failed to open PTY slave {path}: {e} (TIOCGPTPEER: {peer_err})"
            ))
        })
}

fn pty_index(master_fd: RawFd) -> PtyResult<u32> {
    let mut n: libc::c_uint = 0;
    let rc = unsafe { libc::ioctl(master_fd, TIOCGPTN, &mut n) };
    if rc != 0 {
        return Err(PtyError::SpawnFailed(format!(
            "TIOCGPTN failed: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(n)
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
    let rc = unsafe { libc::ioctl(fd, TIOCSWINSZ, &ws) };
    if rc != 0 {
        return Err(PtyError::ResizeFailed(format!(
            "TIOCSWINSZ failed: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(())
}

/// POSIX login-tty sequence, run in the child between fork and exec.
///
/// Only async-signal-safe calls here — no `log`, no malloc-heavy formatting.
fn setup_child_tty(slave_fd: RawFd) -> io::Result<()> {
    let _ = unsafe { libc::setsid() };
    let _ = unsafe { libc::ioctl(slave_fd, TIOCSCTTY, 0) };
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

    #[test]
    fn test_spawn_true_via_pty() {
        let prog = ["/usr/bin/true", "/bin/true"]
            .iter()
            .copied()
            .find(|p| std::path::Path::new(p).exists());
        let Some(prog) = prog else {
            return;
        };
        let (_master, mut child) = spawn_on_pty(prog, PtySize::default(), None).unwrap();
        let status = child.wait().unwrap();
        assert!(
            status.success(),
            "spawned {prog} via PTY but it failed: {status:?}"
        );
    }
}
