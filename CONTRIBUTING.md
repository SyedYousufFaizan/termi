# Contributing to Next-Gen Terminal

Thank you for your interest in contributing! This document provides guidelines and rules for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Rules](#development-rules)
- [Safety Requirements](#safety-requirements)
- [Pull Request Process](#pull-request-process)
- [Testing Requirements](#testing-requirements)
- [Code Style](#code-style)

---

## Code of Conduct

- Be respectful and constructive
- Focus on the code, not the person
- Help others learn and improve

---

## Getting Started

1. Fork the repository
2. Clone your fork
3. Follow [DEVELOPMENT.md](docs/DEVELOPMENT.md) for setup
4. Create a feature branch: `git checkout -b feature/your-feature`
5. Make your changes
6. Submit a pull request

---

## Development Rules

### 🚫 What NOT to Build

The following are explicitly out of scope for this project:

| Feature | Reason |
|---------|--------|
| Full Wayland compositor | Overengineered for terminal app |
| Local AI assistant | Adds 100MB+, feature creep |
| P2P/IPFS distribution | Complex, solve after MVP |
| Syscall interception | Impossible without root |
| Dynamic code loading | Security risk, Play Store violation |

### ✅ What to Focus On

- Stability over features
- Safety over speed
- Simple over clever
- Working over perfect

### Scope Creep Prevention

Before adding a feature, ask:
1. Does this solve a core user problem?
2. Is this needed for MVP?
3. Does this fit the 3-month timeline?
4. Can this wait until after MVP validation?

---

## Safety Requirements

### ⚠️ MANDATORY: JNI Safety Rules

All JNI code MUST follow these rules:

```rust
// ❌ FORBIDDEN: Never use unwrap() on JNI boundary
let result = env.call_method(...).unwrap();  // CRASH RISK!

// ✅ REQUIRED: Always use safe wrappers
let result = safe_call_bool_method(env, obj, "method", sig, &[...])?;

// ✅ REQUIRED: Check exceptions after EVERY call
if check_and_clear_exception(env) {
    return Err(JniErrorCode::JavaException);
}

// ✅ REQUIRED: Validate handles before dereferencing
let ptr = handle_to_ptr::<MyType>(handle)?;
```

### ⚠️ MANDATORY: VFS Capability Checking

All VFS operations MUST check capabilities:

```rust
// ✅ REQUIRED: Check before operation
if !caps.supports(VfsOperation::Chmod) {
    return Err(VfsError::OperationNotSupported { ... });
}

// ❌ FORBIDDEN: Assuming operations work
std::fs::set_permissions(path, perms)?;  // Will fail silently on SAF!
```

### ⚠️ MANDATORY: Session State Tracking

All state-modifying code MUST track session state:

```rust
// ✅ REQUIRED: Transition states explicitly
state.transition_to(SessionState::Checkpointed);

// ✅ REQUIRED: Checkpoint before risky operations
checkpoint_manager.force_checkpoint(&state)?;
```

---

## Pull Request Process

### Before Submitting

- [ ] All tests pass: `cargo test`
- [ ] Code formatted: `cargo fmt`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] No `unwrap()` in JNI code
- [ ] Documentation updated if needed
- [ ] Tested on Android device (not just emulator)

### PR Title Format

```
type(scope): brief description

Examples:
feat(pty): add resize support
fix(vfs): handle SAF permission loss
docs(api): update checkpoint documentation
test(jni): add null handle tests
```

### PR Description Template

```markdown
## What does this PR do?
Brief description of changes.

## Why is this needed?
Problem being solved.

## How was this tested?
- [ ] Unit tests added/updated
- [ ] Tested on emulator
- [ ] Tested on physical device (specify model)

## Checklist
- [ ] No unwrap() in JNI code
- [ ] VFS operations check capabilities
- [ ] Session state tracked correctly
- [ ] Documentation updated
```

---

## Testing Requirements

### Required Tests

1. **Unit Tests** - For all new functions
2. **JNI Safety Tests** - If touching JNI code
3. **Integration Tests** - For end-to-end flows

### Device Testing

Before claiming a feature is complete:

- [ ] Test on Android emulator
- [ ] Test on Samsung device (One UI)
- [ ] Test on Xiaomi device (MIUI) - if available
- [ ] Test with battery optimization ENABLED

### Test Scenarios

For any file operation code:
- [ ] Test on internal storage
- [ ] Test on SAF-mounted external storage
- [ ] Test with permission revoked mid-operation

For any background operation:
- [ ] Test with app in foreground
- [ ] Test with app backgrounded
- [ ] Test with app killed and restored

---

## Code Style

### Rust

Follow `rustfmt` defaults. Key points:

```rust
// Use Result, not panic
pub fn risky_operation() -> Result<T, Error> {
    // ...
}

// Document public APIs
/// Does something important.
///
/// # Errors
/// Returns `Error::NotFound` if path doesn't exist.
pub fn do_something(path: &Path) -> Result<()> {
    // ...
}

// Use meaningful variable names
let checkpoint_path = get_path();  // ✅
let p = get_path();                // ❌
```

### Kotlin

Follow `ktlint` defaults. Key points:

```kotlin
// Use Kotlin idioms
val items = list.filter { it.isValid }  // ✅
val items = ArrayList<Item>()           // ❌ (Java style)

// Null safety
val name = user?.name ?: "Unknown"  // ✅
if (user != null) { ... }           // ✅ when needed

// Scope functions where appropriate
session?.let { checkpoint(it) }
```

### Compose

```kotlin
// State hoisting
@Composable
fun TerminalView(
    lines: List<RenderLine>,      // State passed down
    onInput: (String) -> Unit,    // Events passed up
)

// Remember expensive objects
val renderer = remember { Renderer() }

// Use keys for lists
items(lines, key = { it.id }) { line ->
    // ...
}
```

---

## Questions?

- Check existing issues first
- Ask in discussions for architecture questions
- Tag maintainers for urgent issues

---

## Recognition

Contributors will be:
- Listed in release notes
- Added to CONTRIBUTORS.md (when created)
- Credited in relevant documentation

Thank you for helping make this project better! 🙏
