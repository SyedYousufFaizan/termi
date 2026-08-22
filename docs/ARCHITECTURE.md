# Architecture Overview

This document describes the technical architecture of the Next-Gen Android Terminal.

## System Design

```
┌─────────────────────────────────────────────────────────────────┐
│                     Android App (Kotlin)                        │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │   Compose UI    │  │  TerminalScreen │  │  FileExplorer   │ │
│  │  (LazyColumn)   │  │    + Toolbar    │  │   (Sidebar)     │ │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘ │
│           │                    │                    │           │
│  ┌────────┴────────────────────┴────────────────────┴────────┐ │
│  │                    JNI Bridge Layer                        │ │
│  │           (TerminalEngine.kt <-> libterminal_core.so)     │ │
│  └────────────────────────────┬──────────────────────────────┘ │
└───────────────────────────────┼─────────────────────────────────┘
                                │
┌───────────────────────────────┼─────────────────────────────────┐
│                    Rust Core Library                            │
│  ┌─────────────────┐  ┌──────┴────────┐  ┌─────────────────┐   │
│  │   PTY Module    │  │  JNI Safe     │  │  Session State  │   │
│  │  (portable-pty) │  │  Wrappers     │  │  Checkpointing  │   │
│  └────────┬────────┘  └───────────────┘  └────────┬────────┘   │
│           │                                        │            │
│  ┌────────┴────────┐  ┌─────────────────┐  ┌─────┴──────────┐  │
│  │ Terminal Module │  │   VFS Module    │  │ Metadata Cache │  │
│  │ (ANSI parsing)  │  │ (Mount Table)   │  │   (SQLite)     │  │
│  └─────────────────┘  └────────┬────────┘  └────────────────┘  │
└────────────────────────────────┼────────────────────────────────┘
                                 │
                    ┌────────────┴────────────┐
                    │   Android SAF Bridge    │
                    │   (JNI -> Kotlin SAF)   │
                    └─────────────────────────┘
```

## Module Structure

### Rust Core (`rust/src/`)

```
rust/src/
├── lib.rs              # Library entry point, exports
├── jni_safe.rs         # Safe JNI wrappers (CRITICAL) — split into a pure,
│                       # always-compiled half and a `#[cfg(feature =
│                       # "android")]`-gated half as of Phase 1b, so this
│                       # crate builds/tests without any Android tooling
├── session_state.rs    # Session lifecycle management
├── pty/
│   ├── mod.rs          # PTY module exports
│   ├── core.rs         # PtySession - main PTY interface
│   └── process.rs      # Process spawning config
├── terminal/
│   ├── mod.rs          # Terminal module exports
│   ├── cell.rs         # Cell struct (char + style)
│   ├── screen.rs       # Screen buffer management
│   └── renderer.rs     # Render output for UI
├── vfs/
│   ├── mod.rs          # VFS module exports
│   ├── capabilities.rs # Filesystem capability checking (moved here from
│   │                   # crate root `vfs_capabilities.rs` in Phase 1 cleanup)
│   ├── mount.rs        # Virtual mount table
│   ├── provider.rs     # Filesystem provider trait (chmod/symlink/readlink
│   │                   # added in Phase 1d, defaulting to unsupported)
│   ├── service.rs      # NEW (Phase 1d): VfsService — enforces capabilities
│   │                   # before dispatching to a provider, returns VfsOutcome
│   ├── health.rs       # NEW (Phase 1d): SAF permission health-check state
│   │                   # machine (Valid/Stale/Revoked)
│   ├── cache.rs        # Metadata caching
│   ├── android_saf.rs  # Android SAF implementation (still a stub — see
│   │                   # docs/PHASE1_STATUS.md)
│   └── ios_provider.rs # iOS placeholder — not resourced, see file header
├── utils/
│   ├── mod.rs          # Utils module exports
│   ├── error.rs        # Unified error types
│   ├── sync_ext.rs     # NEW (Phase 1b): LockExt::lock_safe() — poison-safe
│   │                   # mutex locking, replaces .lock().unwrap()
│   └── logger.rs       # Logging setup
└── package/
    ├── mod.rs          # Package manager (future)
    ├── manager.rs      # Package installation
    └── repository.rs   # Package sources
```

### Android App (`android/app/src/main/java/com/terminal/`)

```
com/terminal/
├── MainActivity.kt         # App entry point
├── TerminalApplication.kt  # Application class
├── core/
│   ├── TerminalEngine.kt   # JNI bridge to Rust
│   ├── SessionManager.kt   # Session lifecycle
│   └── TerminalService.kt  # Foreground service
├── ui/
│   ├── components/         # Reusable UI components
│   │   ├── TerminalView.kt
│   │   ├── CommandToolbar.kt
│   │   ├── FileExplorer.kt
│   │   ├── FileItem.kt
│   │   ├── SafWarningDialog.kt
│   │   └── SessionStateBanner.kt
│   ├── screens/
│   │   ├── TerminalScreen.kt
│   │   ├── SettingsScreen.kt
│   │   └── AboutScreen.kt
│   ├── navigation/
│   │   └── NavGraph.kt
│   └── theme/
│       ├── Color.kt
│       ├── Theme.kt
│       └── Type.kt
├── vfs/
│   ├── SafHelper.kt        # SAF operations
│   ├── MountManager.kt     # Mount point UI
│   └── PermissionManager.kt
└── utils/
    ├── Extensions.kt
    ├── Logger.kt
    └── PreferencesManager.kt
```

## Key Components

### 1. JNI Safety Layer (`jni_safe.rs`)

All JNI calls MUST go through this layer. It provides:

- **Exception Checking**: `check_and_clear_exception()` after every Java call
- **Type-Safe Wrappers**: `safe_call_bool_method()`, `safe_call_int_method()`, etc.
- **Handle Management**: `handle_box()`, `handle_drop()` for passing Rust objects to Java
- **Panic Hook**: Prevents Rust panics from unwinding across FFI boundary

```rust
// CORRECT: Use safe wrappers
let result = safe_call_bool_method(env, obj, "exists", "(Ljava/lang/String;)Z", &[...])?;

// WRONG: Raw JNI call without exception check
let result = env.call_method(obj, "exists", ...); // DANGEROUS!
```

### 2. VFS Capability System (`vfs/capabilities.rs` + `vfs/service.rs`)

Tracks what operations are supported on each filesystem:

| Operation | Internal | SAF External |
|-----------|----------|--------------|
| Read/Write | ✅ | ✅ |
| chmod | ✅ | ❌ |
| symlink | ✅ | ❌ |
| inotify | ✅ | ❌ |
| atomic rename | ✅ | ⚠️ |

As of Phase 1d, this table is actually enforced, not just documented:
`VfsService` (in `vfs/service.rs`) checks capabilities before dispatching
to a provider and returns a `VfsOutcome` carrying a user-facing hint for
blocked operations (e.g. "move this project to internal storage"). See
`docs/PHASE1_STATUS.md` and `.cursor/rules/40-vfs-saf-architecture.mdc`
for the full design.

```rust
// Always check before operation
if !vfs.supports(path, VfsOperation::Chmod) {
    return Err(VfsError::OperationNotSupported { ... });
}
```

### 3. Session State Management (`session_state.rs`)

Handles Android's aggressive process killing:

```
Active → Checkpointed → (process killed) → Restored
                                              ↓
                                           Failed
```

- **Active**: Terminal running normally
- **Checkpointed**: State saved to disk (on background)
- **Restored**: State recovered after process death
- **Failed**: Restoration failed

### 4. Virtual Mount Table (`vfs/mount.rs`)

Maps virtual Unix paths to actual storage backends:

```
/                    → Internal storage (full Unix)
/mnt/downloads       → SAF: content://downloads/... (limited)
/mnt/sdcard          → SAF: content://external/... (limited)
```

### 5. Metadata Cache (`vfs/cache.rs`)

SAF operations are slow (50-200ms). The cache provides:

- **TTL-based caching**: 5-second default expiry
- **Invalidation**: On write/delete operations
- **Prefix invalidation**: For directory operations
- **Statistics**: Hit rate monitoring

## Data Flow

### Command Execution

```
1. User types "ls" in UI
2. TerminalView.kt captures input
3. TerminalEngine.kt calls JNI: nativeWrite(handle, "ls\n")
4. jni_safe wrappers validate call
5. PtySession.write() sends to PTY
6. Shell executes, writes output
7. PtySession.read() receives output
8. Terminal parser processes ANSI codes
9. Screen buffer updated
10. Renderer creates RenderOutput
11. JNI returns styled lines to Kotlin
12. Compose UI recomposes LazyColumn
```

### File Access (External Storage)

```
1. User: cat /mnt/downloads/file.txt
2. VFS resolves path → SAF mount
3. Check cache for metadata
4. If miss: JNI → SafHelper.readFile() → ContentResolver
5. Cache result with TTL
6. Return data to shell
```

## Threading Model

```
Main Thread (Android)
├── UI rendering (Compose)
└── JNI calls (blocking, keep fast)

Background Threads
├── PTY I/O (read loop)
├── Checkpoint timer (30s interval)
└── Cache cleanup

Foreground Service Thread
└── Keeps app alive during background
```

## Security Considerations

1. **No Dynamic Code Loading**: All code shipped in APK
2. **No Root Required**: Works on stock devices
3. **SAF Permissions**: User must explicitly grant directory access
4. **Handle Validation**: All JNI handles checked before use
5. **Panic Isolation**: Panics caught at FFI boundary

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Keystroke latency | <16ms | 60 FPS responsiveness |
| Command start | <100ms | Time to first output |
| SAF read (cached) | <1ms | From metadata cache |
| SAF read (uncached) | <200ms | ContentResolver call |
| Memory (idle) | <50MB | Minimal footprint |
| Memory (10K lines) | <150MB | Large scrollback |
| APK size | <15MB | Core app without packages |

## Future Architecture (Phase 2+)

```
┌─────────────────────────────────────────────────────────────┐
│                      Package Manager                         │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐  │
│  │ GitHub Releases │  │  Local Cache    │  │  Integrity  │  │
│  │   (Primary)     │  │  (Downloaded)   │  │  Checking   │  │
│  └─────────────────┘  └─────────────────┘  └─────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

Packages downloaded from GitHub releases, verified, extracted to app storage.
