# API Reference

This document describes the public API of the `terminal_core` Rust library.

## Table of Contents

- [Library Initialization](#library-initialization)
- [JNI Safety Module](#jni-safety-module)
- [Session State](#session-state)
- [VFS Capabilities](#vfs-capabilities)
- [PTY Module](#pty-module)
- [Terminal Module](#terminal-module)
- [VFS Module](#vfs-module)
- [Error Types](#error-types)

---

## Library Initialization

```rust
use terminal_core;

// Initialize the library (call once at startup)
terminal_core::init();

// Get library version
let version = terminal_core::VERSION;
```

---

## JNI Safety Module

**Module:** `terminal_core::jni_safe`

### Error Codes

```rust
#[repr(i32)]
pub enum JniErrorCode {
    Success = 0,
    NullPointer = -1,
    InvalidHandle = -2,
    JavaException = -3,
    InvalidUtf8 = -4,
    InvalidArgument = -5,
    OutOfMemory = -6,
    PtyError = -7,
    VfsError = -8,
    IoError = -9,
    Unknown = -99,
}
```

### Exception Handling

```rust
/// Check and clear Java exceptions after JNI calls
/// Returns true if an exception was pending
pub fn check_and_clear_exception(env: &mut JNIEnv) -> bool;
```

### Safe Method Calls

```rust
/// Call a Java method that returns void
pub fn safe_call_void_method(
    env: &mut JNIEnv,
    obj: &JObject,
    method_name: &str,
    sig: &str,
    args: &[JValue],
) -> JniResult<()>;

/// Call a Java method that returns boolean
pub fn safe_call_bool_method(...) -> JniResult<bool>;

/// Call a Java method that returns int
pub fn safe_call_int_method(...) -> JniResult<i32>;

/// Call a Java method that returns long
pub fn safe_call_long_method(...) -> JniResult<i64>;
```

### String Operations

```rust
/// Convert JString to Rust String
pub fn safe_get_string(env: &mut JNIEnv, s: &JString) -> JniResult<String>;

/// Create JString from Rust string
pub fn safe_new_string<'a>(env: &mut JNIEnv<'a>, s: &str) -> JniResult<JString<'a>>;
```

### Handle Management

```rust
/// Box a value and return handle for Java
pub fn handle_box<T>(value: T) -> jlong;

/// Drop a boxed value from handle (MUST be called exactly once)
pub unsafe fn handle_drop<T>(handle: jlong) -> JniResult<()>;

/// Get immutable reference from handle
pub unsafe fn handle_to_ref<'a, T>(handle: jlong) -> JniResult<&'a T>;

/// Get mutable reference from handle
pub unsafe fn handle_to_mut<'a, T>(handle: jlong) -> JniResult<&'a mut T>;
```

### Panic Hook

```rust
/// Install panic hook to prevent unwinding across FFI
/// Call once at library initialization
pub fn install_panic_hook();
```

---

## Session State

**Module:** `terminal_core::session_state`

### SessionState Enum

```rust
#[repr(u8)]
pub enum SessionState {
    Active = 0,        // Running normally
    Checkpointed = 1,  // Saved to disk
    Restored = 2,      // Recovered from checkpoint
    Failed = 3,        // Restoration failed
}

impl SessionState {
    /// Human-readable message for UI
    pub fn display_message(&self) -> &'static str;
    
    /// Color for UI indicator (ARGB)
    pub fn indicator_color(&self) -> u32;
    
    /// Whether state requires user attention
    pub fn needs_attention(&self) -> bool;
}
```

### TerminalState Struct

```rust
pub struct TerminalState {
    pub session_id: String,
    pub state: SessionState,
    pub screen_buffer: Vec<ScreenLine>,
    pub cursor_position: (u32, u32),
    pub cwd: String,
    pub env_vars: Vec<(String, String)>,
    pub scrollback_size: usize,
    pub dimensions: (u16, u16),
    pub checkpoint_time: u64,
    pub version: u32,
}

impl TerminalState {
    pub fn new(session_id: impl Into<String>) -> Self;
    pub fn transition_to(&mut self, new_state: SessionState);
}
```

### CheckpointManager

```rust
pub struct CheckpointManager {
    // ...
}

impl CheckpointManager {
    /// Create new manager with checkpoint directory
    pub fn new(checkpoint_dir: impl Into<PathBuf>) -> Self;
    
    /// Set minimum checkpoint interval (default: 30s)
    pub fn with_interval(self, interval: Duration) -> Self;
    
    /// Check if checkpoint is due
    pub fn should_checkpoint(&self) -> bool;
    
    /// Force immediate checkpoint
    pub fn force_checkpoint(&mut self, state: &TerminalState) -> Result<(), CheckpointError>;
    
    /// Checkpoint if interval elapsed
    pub fn maybe_checkpoint(&mut self, state: &TerminalState) -> Result<bool, CheckpointError>;
    
    /// Restore session from checkpoint
    pub fn restore(&self, session_id: &str) -> Result<TerminalState, CheckpointError>;
    
    /// List all checkpoints
    pub fn list_checkpoints(&self) -> Vec<CheckpointInfo>;
    
    /// Delete a checkpoint
    pub fn delete_checkpoint(&self, session_id: &str) -> Result<(), CheckpointError>;
    
    /// Clean up old checkpoints (keep N most recent)
    pub fn cleanup(&self, keep_count: usize) -> Result<usize, CheckpointError>;
}
```

---

## VFS Capabilities

**Module:** `terminal_core::vfs::capabilities` (moved from the crate-root
`vfs_capabilities` module in the Phase 1 repo cleanup — see
[PHASE1_STATUS.md](PHASE1_STATUS.md))

### VfsOperation Enum

```rust
#[repr(u8)]
pub enum VfsOperation {
    Read = 0,
    Write = 1,
    Create = 2,
    Delete = 3,
    Rename = 4,
    Chmod = 5,
    Chown = 6,
    Symlink = 7,
    Hardlink = 8,
    ListDir = 9,
    Mkdir = 10,
    Stat = 11,
    SetTimestamp = 12,
    Watch = 13,
    AtomicRename = 14,
    Mmap = 15,
    Lock = 16,
    Xattr = 17,
}
```

### VfsCapabilities Struct

```rust
pub struct VfsCapabilities {
    pub supported: HashSet<VfsOperation>,
    pub is_saf: bool,
    pub fs_type: FsType,
    pub max_filename_len: usize,
    pub max_path_len: usize,
    pub read_only: bool,
    pub case_sensitive: bool,
}

impl VfsCapabilities {
    /// Full Unix support (internal storage)
    pub fn internal_storage() -> Self;
    
    /// Limited SAF support (external storage)
    pub fn saf_external() -> Self;
    
    /// FAT32/exFAT support
    pub fn fat_external() -> Self;
    
    /// Check if operation is supported
    pub fn supports(&self, op: VfsOperation) -> bool;
    
    /// Get unsupported operations list
    pub fn unsupported_operations(&self) -> Vec<VfsOperation>;
    
    /// Generate warning message for UI
    pub fn limitation_warning(&self) -> Option<String>;
}
```

### Tool Compatibility

```rust
pub struct ToolCompatibility;

impl ToolCompatibility {
    /// Check tool compatibility with filesystem
    pub fn check(tool: &str, caps: &VfsCapabilities) -> ToolCompatibilityResult;
}

pub enum ToolCompatibilityResult {
    FullyCompatible,
    PartiallyCompatible { tool: String, issues: Vec<String>, recommendation: String },
    NotCompatible { tool: String, reason: String, recommendation: String },
    Unknown,
}
```

---

## PTY Module

**Module:** `terminal_core::pty`

### PtySession

```rust
pub struct PtySession {
    // ...
}

impl PtySession {
    /// Create new PTY session
    pub fn new(session_id: impl Into<String>) -> PtyResult<Self>;
    
    /// Spawn shell process
    pub fn spawn_shell(&mut self, shell_path: &str) -> PtyResult<()>;
    
    /// Write data to PTY
    pub fn write(&mut self, data: &[u8]) -> PtyResult<usize>;
    
    /// Read data from PTY
    pub fn read(&mut self, buf: &mut [u8]) -> PtyResult<usize>;
    
    /// Resize PTY
    pub fn resize(&mut self, cols: u16, rows: u16) -> PtyResult<()>;
    
    /// Get session state
    pub fn session_state(&self) -> SessionState;
    
    /// Get terminal state for checkpointing
    pub fn terminal_state(&self) -> Option<TerminalState>;
    
    /// Check if running
    pub fn is_running(&self) -> bool;
    
    /// Get exit code
    pub fn exit_code(&self) -> Option<i32>;
    
    /// Send signal
    pub fn signal(&mut self, signal: i32) -> PtyResult<()>;
    
    /// Close PTY
    pub fn close(&mut self) -> PtyResult<()>;
}
```

### ProcessConfig

```rust
pub struct ProcessConfig {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: PathBuf,
    pub size: (u16, u16),
}

impl ProcessConfig {
    pub fn new() -> Self;
    pub fn program(self, program: impl Into<PathBuf>) -> Self;
    pub fn arg(self, arg: impl Into<String>) -> Self;
    pub fn args(self, args: impl IntoIterator<Item = impl Into<String>>) -> Self;
    pub fn env(self, key: impl Into<String>, value: impl Into<String>) -> Self;
    pub fn cwd(self, cwd: impl Into<PathBuf>) -> Self;
    pub fn size(self, cols: u16, rows: u16) -> Self;
    pub fn validate(&self) -> PtyResult<()>;
}
```

### Signals

```rust
pub mod signals {
    pub const SIGHUP: i32 = 1;
    pub const SIGINT: i32 = 2;
    pub const SIGQUIT: i32 = 3;
    pub const SIGKILL: i32 = 9;
    pub const SIGTERM: i32 = 15;
    pub const SIGCONT: i32 = 18;
    pub const SIGSTOP: i32 = 19;
    pub const SIGWINCH: i32 = 28;
}
```

---

## Terminal Module

**Module:** `terminal_core::terminal`

### Cell

```rust
pub struct Cell {
    pub c: char,       // Character
    pub fg: u32,       // Foreground color (ARGB)
    pub bg: u32,       // Background color (ARGB)
    pub attrs: CellAttrs,
}

pub struct CellAttrs {
    // Packed attributes
}

impl CellAttrs {
    pub fn bold(&self) -> bool;
    pub fn italic(&self) -> bool;
    pub fn underline(&self) -> bool;
    pub fn strikethrough(&self) -> bool;
    pub fn inverse(&self) -> bool;
    // ... setters
}

// Default colors
pub const DEFAULT_FG: u32 = 0xFFFFFFFF;
pub const DEFAULT_BG: u32 = 0xFF000000;

pub mod colors {
    pub const BLACK: u32 = 0xFF000000;
    pub const RED: u32 = 0xFFCD0000;
    // ... full ANSI palette
    
    /// Get color by ANSI index (0-255)
    pub fn ansi_color(index: u8) -> u32;
}
```

### Screen

```rust
pub struct Screen {
    // ...
}

impl Screen {
    pub fn new(cols: usize, rows: usize) -> Self;
    pub fn size(&self) -> (usize, usize);
    pub fn cursor(&self) -> (usize, usize);
    pub fn set_cursor(&mut self, row: usize, col: usize);
    pub fn move_cursor(&mut self, delta_row: i32, delta_col: i32);
    pub fn get_cell(&self, row: usize, col: usize) -> Option<&Cell>;
    pub fn write_char(&mut self, c: char);
    pub fn newline(&mut self);
    pub fn scroll_up(&mut self);
    pub fn clear(&mut self);
    pub fn clear_to_end(&mut self);
    pub fn clear_line(&mut self);
    pub fn set_attrs(&mut self, attrs: CellAttrs);
    pub fn set_fg(&mut self, color: u32);
    pub fn set_bg(&mut self, color: u32);
    pub fn reset_attrs(&mut self);
    pub fn resize(&mut self, new_cols: usize, new_rows: usize);
    pub fn scrollback_len(&self) -> usize;
    pub fn to_text(&self) -> String;  // For debugging
}
```

### Renderer

```rust
pub struct RenderLine {
    pub text: String,
    pub spans: Vec<StyleSpan>,
}

pub struct StyleSpan {
    pub start: usize,
    pub end: usize,
    pub fg: u32,
    pub bg: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

pub struct RenderOutput {
    pub lines: Vec<RenderLine>,
    pub cursor: (usize, usize),
    pub cursor_visible: bool,
    pub total_lines: usize,
    pub scroll_offset: usize,
}

pub struct Renderer {
    // ...
}

impl Renderer {
    pub fn new() -> Self;
    pub fn toggle_cursor_blink(&mut self);
    pub fn set_scroll_offset(&mut self, offset: usize);
    pub fn scroll_up(&mut self, lines: usize, max_scrollback: usize);
    pub fn scroll_down(&mut self, lines: usize);
    pub fn scroll_to_bottom(&mut self);
    pub fn render(&self, screen: &Screen) -> RenderOutput;
}
```

---

## VFS Module

**Module:** `terminal_core::vfs`

### MountPoint

```rust
pub struct MountPoint {
    pub virtual_path: PathBuf,
    pub source: MountSource,
    pub capabilities: VfsCapabilities,
    pub active: bool,
    pub display_name: String,
}

pub enum MountSource {
    Internal(PathBuf),
    SafUri(String),
}

impl MountPoint {
    pub fn internal(virtual_path: impl Into<PathBuf>, real_path: impl Into<PathBuf>) -> Self;
    pub fn saf(virtual_path: impl Into<PathBuf>, uri: impl Into<String>, display_name: impl Into<String>) -> Self;
    pub fn supports(&self, op: VfsOperation) -> bool;
    pub fn limitation_warning(&self) -> Option<String>;
}
```

### MountTable

```rust
pub struct MountTable {
    // ...
}

impl MountTable {
    pub fn new(internal_root: impl Into<PathBuf>) -> Self;
    pub fn mount(&mut self, mount: MountPoint) -> VfsResult<()>;
    pub fn unmount(&mut self, virtual_path: &Path) -> VfsResult<()>;
    pub fn find_mount(&self, virtual_path: &Path) -> Option<&MountPoint>;
    pub fn resolve(&self, virtual_path: &Path) -> VfsResult<(&MountPoint, PathBuf)>;
    pub fn get_capabilities(&self, virtual_path: &Path) -> VfsCapabilities;
    pub fn supports_operation(&self, virtual_path: &Path, op: VfsOperation) -> bool;
    pub fn list_mounts(&self) -> Vec<&MountPoint>;
}
```

### FsProvider Trait

```rust
pub trait FsProvider: Send + Sync {
    fn read_file(&self, path: &Path) -> VfsResult<Vec<u8>>;
    fn write_file(&self, path: &Path, contents: &[u8]) -> VfsResult<()>;
    fn metadata(&self, path: &Path) -> VfsResult<FileMetadata>;
    fn list_dir(&self, path: &Path) -> VfsResult<Vec<DirEntry>>;
    fn create_dir(&self, path: &Path) -> VfsResult<()>;
    fn delete(&self, path: &Path) -> VfsResult<()>;
    fn rename(&self, from: &Path, to: &Path) -> VfsResult<()>;
    fn exists(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;

    // Added in Phase 1d. All three default to `Err(OperationNotSupported)`
    // — only override them if the backend genuinely supports the
    // operation (InternalProvider overrides all three with real Unix
    // syscalls; SafProvider intentionally overrides none of them).
    fn chmod(&self, path: &Path, mode: u32) -> VfsResult<()> { /* default: unsupported */ }
    fn symlink(&self, target: &Path, link: &Path) -> VfsResult<()> { /* default: unsupported */ }
    fn readlink(&self, path: &Path) -> VfsResult<PathBuf> { /* default: unsupported */ }
}
```

### VfsService (new in Phase 1d)

**Module:** `terminal_core::vfs::service`

The facade that enforces the capability system before dispatching to a
provider. This is what every real filesystem operation should go
through — calling a `FsProvider` method directly bypasses the capability
check entirely.

```rust
pub enum VfsOutcome<T> {
    Ok(T),
    Blocked { operation: VfsOperation, reason: String, hint: Option<String> },
    Degraded { value: T, caveat: String },
}

pub struct VfsService { /* ... */ }

impl VfsService {
    pub fn new(internal_root: impl Into<PathBuf>, internal_provider: Arc<dyn FsProvider>) -> Self;
    pub fn mount_provider(&mut self, mount: MountPoint, provider: Arc<dyn FsProvider>) -> VfsResult<()>;
    pub fn unmount(&mut self, virtual_path: &Path) -> VfsResult<()>;
    pub fn mounts(&self) -> &MountTable;

    pub fn read_file(&self, path: &Path) -> VfsOutcome<Vec<u8>>;
    pub fn write_file(&self, path: &Path, contents: &[u8]) -> VfsOutcome<()>;
    pub fn metadata(&self, path: &Path) -> VfsOutcome<FileMetadata>;
    pub fn list_dir(&self, path: &Path) -> VfsOutcome<Vec<DirEntry>>;
    pub fn create_dir(&self, path: &Path) -> VfsOutcome<()>;
    pub fn delete(&self, path: &Path) -> VfsOutcome<()>;
    pub fn chmod(&self, path: &Path, mode: u32) -> VfsOutcome<()>;
    pub fn symlink(&self, target: &Path, link: &Path) -> VfsOutcome<()>;
    pub fn readlink(&self, path: &Path) -> VfsOutcome<PathBuf>;
    pub fn rename(&self, from: &Path, to: &Path) -> VfsOutcome<()>;
}
```

`VfsOutcome::Blocked.hint` is intended to be rendered directly as an
inline terminal warning (e.g. "Move this project to internal storage to
use this feature") rather than a bare error — see
`.cursor/rules/40-vfs-saf-architecture.mdc` for the design rationale.

### HealthMonitor (new in Phase 1d)

**Module:** `terminal_core::vfs::health`

SAF permission staleness/revocation detection, independent of the
capability system above (capability = "can this filesystem type do X at
all"; health = "has this specific mount's access grant gone bad").

```rust
pub enum PermissionState { Valid, Stale, Revoked, NotApplicable }

pub trait PermissionProbe: Send + Sync {
    fn check(&self, mount: &MountPoint) -> PermissionState;
}

pub struct HealthMonitor { /* ... */ }

impl HealthMonitor {
    pub fn new(probe: Box<dyn PermissionProbe>) -> Self;
    pub fn always_valid() -> Self;
    pub fn scan(&self, table: &MountTable) -> Vec<MountHealth>;
    pub fn scan_needs_attention(&self, table: &MountTable) -> Vec<MountHealth>;
}
```

The real Android-backed `PermissionProbe` implementation (JNI calls into
`ContentResolver`) is not yet written — see
`.cursor/skills/wire-permission-health-check.md`. Everything above is
tested on host with a `FakeProbe` standing in for the real JNI call.

### MetadataCache

```rust
pub struct MetadataCache {
    // ...
}

impl MetadataCache {
    pub fn new() -> Self;
    pub fn with_ttl(ttl: Duration) -> Self;
    pub fn with_capacity(max_entries: usize) -> Self;
    pub fn get(&mut self, path: &Path) -> Option<FileMetadata>;
    pub fn insert(&mut self, path: &Path, metadata: FileMetadata);
    pub fn invalidate(&mut self, path: &Path);
    pub fn invalidate_prefix(&mut self, prefix: &Path);
    pub fn clear(&mut self);
    pub fn stats(&self) -> &CacheStats;
    pub fn evict_expired(&mut self) -> usize;
}

pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub insertions: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64;
}
```

---

## Error Types

**Module:** `terminal_core::utils::error`

```rust
/// Top-level error type
pub enum TerminalError {
    Pty(PtyError),
    Vfs(VfsError),
    Session(SessionError),
    Jni(JniError),
    Io(std::io::Error),
}

pub enum PtyError {
    SpawnFailed(String),
    ProcessExited(i32),
    ReadFailed(String),
    WriteFailed(String),
    ResizeFailed(String),
    InvalidHandle,
    NotInitialized,
    SignalFailed { signal: i32, reason: String },
}

pub enum VfsError {
    NotFound(String),
    PermissionDenied(String),
    OperationNotSupported { operation: VfsOperation, reason: String },
    NotMounted(String),
    MountFailed { path: String, reason: String },
    InvalidUri(String),
    PermissionLost(String),
    CacheError(String),
    AlreadyExists(String),
    NotADirectory(String),
    NotAFile(String),
    DirectoryNotEmpty(String),
}

pub enum SessionError {
    NotFound(String),
    AlreadyExists(String),
    InvalidState { current: SessionState, operation: String },
    Checkpoint(CheckpointError),
    RestoreFailed(String),
    LimitReached(usize),
}

// Result type aliases
pub type Result<T> = std::result::Result<T, TerminalError>;
pub type PtyResult<T> = std::result::Result<T, PtyError>;
pub type VfsResult<T> = std::result::Result<T, VfsError>;
pub type SessionResult<T> = std::result::Result<T, SessionError>;
```
