---
name: "add-vfs-provider"
description: "Worked example for adding a new VFS/FsProvider backend (e.g. an SSH remote provider for Phase 2), including how to register it with VfsService and write a test double. Use when adding or reviewing a new storage backend."
icon: "git-branch"
color: "blue"
---

Worked example based on how `InternalProvider` and the test-only
`MockSafProvider` are structured. Follow this shape for any new storage
backend (SSH/SFTP, a future cloud-sync provider, etc.).

## 1. Implement the `FsProvider` trait

```rust
// rust/src/vfs/ssh_provider.rs (example — doesn't exist yet)
use crate::vfs::provider::{DirEntry, FileMetadata, FsProvider};
use crate::utils::error::{VfsError, VfsResult};
use std::path::Path;

pub struct SshProvider {
    // connection state, session handle, etc.
}

impl FsProvider for SshProvider {
    fn read_file(&self, path: &Path) -> VfsResult<Vec<u8>> { /* SFTP read */ todo!() }
    fn write_file(&self, path: &Path, contents: &[u8]) -> VfsResult<()> { todo!() }
    fn metadata(&self, path: &Path) -> VfsResult<FileMetadata> { todo!() }
    fn list_dir(&self, path: &Path) -> VfsResult<Vec<DirEntry>> { todo!() }
    fn create_dir(&self, path: &Path) -> VfsResult<()> { todo!() }
    fn delete(&self, path: &Path) -> VfsResult<()> { todo!() }
    fn rename(&self, from: &Path, to: &Path) -> VfsResult<()> { todo!() }
    fn exists(&self, path: &Path) -> bool { todo!() }
    fn is_dir(&self, path: &Path) -> bool { todo!() }

    // Only override chmod/symlink if SFTP genuinely supports them for
    // this backend — many SFTP servers DO support chmod (unlike SAF), so
    // this is a case where you'd actually implement it rather than
    // inheriting the "not supported" default:
    fn chmod(&self, path: &Path, mode: u32) -> VfsResult<()> {
        // real SFTP SETSTAT call
        todo!()
    }
    // Don't override symlink/readlink unless the SSH server's filesystem
    // actually supports it — check before assuming.
}
```

**Don't forget `#[warn(clippy::unwrap_used)]` compliance** — no
`.unwrap()` in the real implementation, propagate `VfsError` via `?`.

## 2. Register it with `VfsService`, not standalone

```rust
let mount = MountPoint::remote("/mnt/home-server", "ssh://user@host", "Home Server");
let provider: Arc<dyn FsProvider> = Arc::new(SshProvider::connect(...)?);
vfs_service.mount_provider(mount, provider)?;
```

Never call `SshProvider` methods directly from application code — always
go through `vfs_service.read_file(...)` etc., so the capability check in
`vfs::service` applies uniformly. See
`.cursor/rules/40-vfs-saf-architecture.mdc` for why this matters.

## 3. Add a capability profile if the defaults don't fit

Check `vfs/capabilities.rs` for the existing `FsType`/`VfsCapabilities`
variants (`internal()`, `saf_external()`). If SSH-backed storage has a
genuinely different capability profile (e.g. supports chmod but not
mmap), add a new `VfsCapabilities::ssh_remote()` constructor there
following the existing pattern, rather than hacking around it in the
provider.

## 4. Write a test double, not just relying on the real backend

Follow the `MockSafProvider` pattern in `vfs/service.rs`'s test module: an
in-memory `FsProvider` impl that mirrors the real backend's capability
profile (overrides the same methods the real one would, no more, no
less). This is what lets capability-blocking behavior be tested without a
live SSH connection or test server.

```rust
struct MockSshProvider { files: Mutex<HashMap<PathBuf, Vec<u8>>> }
impl FsProvider for MockSshProvider {
    // ...same shape as MockSafProvider, but override chmod since SSH
    // (unlike SAF) actually supports it — this asymmetry is exactly what
    // you want your tests to prove is handled correctly.
}
```

## 5. Tests to write (minimum bar, mirrors `vfs/service.rs`)

- One test per `VfsOutcome` variant your new provider can produce for each
  operation (e.g. chmod → `Ok` since SSH supports it, unlike the SAF
  `Blocked` case).
- A cross-mount test if relevant (e.g. rename between SSH and internal —
  currently unsupported everywhere, should return `Blocked` with a clear
  reason, not panic or silently do the wrong thing).
- Run `cargo test` — no Android SDK/NDK needed for any of this, it's all
  host-testable by design.
