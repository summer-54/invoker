# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

Invoker is a competitive programming judge system written in Rust that executes and evaluates solutions in sandboxed environments. It uses WebSocket communication for real-time interaction and integrates with the `isolate` sandbox system for secure code execution.

### Architecture

- **Main Application (`main.rs`)**: Orchestrates the judging process using WebSocket communication
- **Judge Service (`judge.rs`)**: Core judging logic that compiles and tests solutions against test cases
- **Sandbox Integration (`sandboxes/isolate.rs`)**: Manages secure code execution via the `isolate` tool
- **WebSocket Communication (`ws.rs`)**: Handles real-time messaging protocol for judge operations
- **API Layer (`api.rs`)**: Defines message types for incoming and outgoing communications

### Key Components

- **Async Architecture**: Built on Tokio for concurrent test execution and WebSocket handling
- **Sandboxing**: Uses Linux `isolate` tool for secure, resource-limited code execution
- **Problem Management**: Supports tar-compressed problem packages with configurable test groups
- **Multi-language Support**: Currently supports C++ (g++) and Python3 compilation/execution

## Development Commands

### Build and Run
```bash
# Build the project
cargo build

# Build for release
cargo build --release

# Run with environment variables (example)
INVOKER_MANAGER_HOST=127.0.0.1:5477 INVOKER_CONFIG_DIR=.config/invoker INVOKER_WORK_DIR=invoker INVOKER_ISOLATE_EXE_PATH=.local/bin/isolate cargo run

# Run with debug logging
RUST_LOG=debug cargo run
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test module
cargo test judge::parsing
cargo test archive::compression
cargo test sandboxes::isolate

# Run tests with output
cargo test -- --nocapture
```

### Development Features
```bash
# Enable mock features for testing
cargo build --features mock

# Check for compilation issues
cargo check

# Format code
cargo fmt

# Run clippy for linting
cargo clippy
```

## Environment Configuration

Required environment variables:
- `INVOKER_MANAGER_HOST`: WebSocket server address (e.g., `127.0.0.1:5477`)
- `INVOKER_CONFIG_DIR`: Configuration directory path (e.g., `.config/invoker`)
- `INVOKER_WORK_DIR`: Working directory for judge operations (e.g., `invoker`)
- `INVOKER_ISOLATE_EXE_PATH`: Path to isolate executable (e.g., `.local/bin/isolate`)

## Problem Templates

The system expects problem packages as tar archives containing:
- `config.yaml`: Problem configuration with limits, groups, and language settings
- `checker.out`: Executable checker program
- `solution`: Solution source code to be tested
- `correct/`: Directory with expected outputs (optional)
- `input/`: Directory with test inputs (optional for some problem types)

### Problem Configuration Structure
```yaml
type: standart
lang: g++
limits:
  time: 2000        # milliseconds
  real_time: 2000   # milliseconds
  memory: 512000    # KB
  stack: 512000     # KB (optional)
groups:
  - id: 0
    range: [1, 2]   # test range [inclusive]
    cost: 0         # points for this group
    depends: []     # prerequisite groups
```

## WebSocket Protocol

### Incoming Messages
- `START + binary data`: Begin judging with problem package
- `STOP`: Stop current judging process
- `CLOSE`: Shutdown judge connection

### Outgoing Messages
- `TOKEN + UUID`: Judge identification token
- `VERDICT + results`: Final judgment with scores
- `TEST + results`: Individual test case results
- `ERROR/OPERROR`: Error reporting

## Architecture Notes

### Concurrent Testing
- Tests within groups execute concurrently using Tokio spawn
- Failed tests block dependent groups from execution
- Semaphore ensures only one judging session runs at a time

### Sandbox Management
- Each test gets its own isolated sandbox instance
- Configurable resource limits (time, memory, processes, files)
- Automatic cleanup after test completion

### Compilation Support
- Multi-language compilation with configurable commands
- Separate compilation and execution phases
- Compilation errors reported as CE verdicts

## File Locations

- Main source: `src/`
- Problem templates: `templates/problems/`
- Build scripts: `templates/problems/build.sh`
- Isolate config: Generated at `/usr/local/etc/isolate` and `{config_dir}/isolate.yaml`

## Dependencies

Key external dependencies:
- `isolate`: Linux sandboxing tool (must be installed separately)
- `ratchet_rs`: WebSocket implementation with deflate compression
- `tokio-tar`: Async tar archive handling
- `serde_yml`: YAML configuration parsing
