# Rust Workspace Dependency Management

## Overview
This document explains how the slab.rs Rust workspace properly manages dependencies and uses Cargo's build system instead of shell scripts.

## Workspace Structure

### Root Cargo.toml
```toml
[workspace]
resolver = "2"
members = [
    "slab-app/src-tauri",
    "slab-diffusion",
    "slab-diffusion-sys",
    "slab-libfetch",
    "slab-llama",
    "slab-llama-sys",
    "slab-whisper",
    "slab-whisper-sys",
    "slab-core",
    "slab-server",
]

[workspace.dependencies]
# All shared dependencies defined here
anyhow = "1.0.101"
bindgen = "0.72.1"
flate2 = "1.1.9"
libloading = "0.8.9"
reqwest = { version = "0.13.2", features = ["stream", "native-tls", "json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tar = "0.4.44"
tokio = { version = "1.49.0", features = ["full"] }
zip = { version = "4.6.1", default-features = false, features = ["deflate"] }
# ... more dependencies
```

### Member Crate Cargo.toml
```toml
[package]
name = "slab-libfetch"
version.workspace = true
edition.workspace = true
description.workspace = true
repository.workspace = true
keywords.workspace = true
categories.workspace = true
license.workspace = true

[dependencies]
anyhow = { workspace = true }
flate2 = { workspace = true }
reqwest = { workspace = true }
# ... etc
```

## Key Benefits of Workspace Dependencies

### 1. **Version Consistency**
All crates use the same version of dependencies. No more "it works on my machine" due to version mismatches.

### 2. **Simplified Maintenance**
Update dependency versions in ONE place (root Cargo.toml) - all member crates benefit automatically.

### 3. **Faster Builds**
Cargo builds shared dependencies ONCE for the entire workspace, then reuses them across all crates.

### 4. **No Shell Scripts Needed**
Everything is handled by Rust's native build system:
- ✅ Compilation with `cargo build`
- ✅ Testing with `cargo test`
- ✅ Dependency resolution with `cargo check`
- ✅ Release builds with `cargo build --release`
- ✅ Cross-compilation with `--target` flag

## Building the Workspace

### Development Build
```bash
# Build entire workspace in debug mode
cargo build

# Build specific crate
cargo build -p slab-libfetch

# Check compilation without building
cargo check -p slab-libfetch
```

### Release Build
```bash
# Build optimized release binary
cargo build -p slab-libfetch --release

# Binary location: ./target/release/slab-libfetch
```

### Running the Binary
```bash
# Run directly with cargo
cargo run -p slab-libfetch --release

# Or run the built binary
./target/release/slab-libfetch
```

## Cross-Platform Support

### What Works Automatically
- ✅ Linux (x86_64, ARM64)
- ✅ macOS (Intel, Apple Silicon)
- ✅ Windows (x86_64, ARM64)

### Platform Detection
The Rust code uses conditional compilation:
```rust
let os = if cfg!(target_os = "linux") { "linux" }
    else if cfg!(target_os = "macos") { "macos" }
    else if cfg!(target_os = "windows") { "windows" }
    else { "unknown" };

let arch = if cfg!(target_arch = "x86_64") { "x86_64" }
    else if cfg!(target_arch = "aarch64") { "aarch64" }
    else { "unknown" };
```

### Cross-Compilation
```bash
# Build for Windows from Linux
cargo build -p slab-libfetch --target x86_64-pc-windows-gnu --release

# Build for macOS from Linux
cargo build -p slab-libfetch --target x86_64-apple-darwin --release

# Build for ARM64 Linux
cargo build -p slab-libfetch --target aarch64-unknown-linux-gnu --release
```

## Why We Don't Need Shell Scripts

### ❌ Before: Shell Script Approach
```bash
#!/bin/bash
# slab-libfetch.sh - Spawns external processes
curl -L -o "$WHISPER_DIR/$WHISPER_FILE" "$WHISPER_URL"
tar -xzf "$TEMP_DIR/llama.tar.gz" -C "$TEMP_DIR"
unzip -o "$TEMP_DIR/stable-diffusion.zip" -d "$TEMP_DIR"
```

**Problems:**
- ❌ Not cross-platform (different tools on Linux/macOS/Windows)
- ❌ Fragile error handling (exit codes only)
- ❌ No type safety
- ❌ Hard to test
- ❌ External dependencies (curl, tar, unzip must be installed)
- ❌ Slow (process spawning overhead)

### ✅ After: Pure Rust Approach
```rust
// slab-libfetch - Uses native Rust crates
let response = client.get(url).send().await?;
let bytes = response.bytes().await?;

// Extract tar.gz
let decoder = GzDecoder::new(cursor);
let archive = Archive::new(decoder);
archive.unpack(dest_dir)?;

// Extract zip
let zip_archive = ZipArchive::new(cursor)?;
```

**Benefits:**
- ✅ Cross-platform (same code runs everywhere)
- ✅ Rich error handling with Result types
- ✅ Type-safe compilation
- ✅ Easy to test (unit tests, integration tests)
- ✅ No external dependencies (all Rust code)
- ✅ Fast (no process spawning)
- ✅ Async/await for better performance

## Dependency Management Best Practices

### DO ✅
1. Define shared dependencies in workspace root
2. Use `{ workspace = true }` in member crates
3. Keep version numbers consistent
4. Use cargo's build system for everything
5. Leverage conditional compilation for platform-specific code
6. Write pure Rust code instead of spawning shell commands

### DON'T ❌
1. Duplicate dependencies across crates
2. Use shell scripts for build tasks
3. Spawn external processes for tasks Rust can do
4. Ignore workspace dependency conventions
5. Mix package managers (npm, pip, etc.) in Rust build

## Build System Features

### Cargo Profiles
```toml
[profile.dev]
opt-level = 0
debug = true

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```

### Feature Flags
```toml
[dependencies]
reqwest = { workspace = true, features = ["json", "stream"] }
tokio = { workspace = true, features = ["full"] }
```

### Build Scripts
```toml
[build-dependencies]
bindgen = { workspace = true }
```

## Testing

### Unit Tests
```bash
cargo test -p slab-libfetch
```

### Integration Tests
```bash
cargo test --workspace
```

### Documentation Tests
```bash
cargo test --doc
```

## CI/CD Integration

### GitHub Actions Example
```yaml
- name: Build slab-libfetch
  run: cargo build -p slab-libfetch --release

- name: Test workspace
  run: cargo test --workspace

- name: Cross-compile for Windows
  run: cargo build -p slab-libfetch --target x86_64-pc-windows-gnu --release
```

## Summary

By leveraging Rust's workspace dependency management and build system:
- ✅ No shell scripts required
- ✅ All dependencies managed by Cargo
- ✅ Cross-platform support built-in
- ✅ Fast, reliable builds
- ✅ Type-safe, testable code
- ✅ Easy maintenance and updates

**The Rust build system handles everything - no external tools needed!**
