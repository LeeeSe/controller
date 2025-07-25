# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust application that uses an Xbox controller (specifically Black Samurai 4 Pro) to control macOS system cursor and perform system operations. The application features automatic reconnection functionality to handle controller sleep/disconnect scenarios.

## Architecture

The application follows a modular architecture with these main components:

1. **Main Module** (`src/main.rs`) - Entry point with control loop and main application logic
2. **HID Module** (`src/hid.rs`) - Low-level HID device communication and state parsing
3. **Input Handler** (`src/input_handler.rs`) - Maps controller inputs to system actions
4. **Connection Manager** (`src/connection_manager.rs`) - Manages device connections and automatic reconnection
5. **Configuration** (`src/config.rs`) - Configuration management and validation
6. **Error Handling** (`src/error.rs`) - Custom error types and recovery strategies

### Key Features
- Left joystick controls mouse cursor movement
- Right joystick up/down for smooth scrolling
- Right joystick left/right for browser navigation
- LT + gyroscope for precise cursor control
- Button mapping for mouse clicks and system functions
- Automatic reconnection when controller disconnects

## Common Development Tasks

### Building and Running
```bash
# Check code compilation
cargo check

# Build debug version
cargo build

# Build release version
cargo build --release

# Run the application
cargo run

# Run tests
cargo test
```

Alternative build script:
```bash
# Using the build script
./build.sh check    # Check compilation
./build.sh build    # Build debug version
./build.sh release  # Build release version
./build.sh test     # Run tests
./build.sh lint     # Code linting
./build.sh format   # Format code
```

### Configuration
The application uses TOML configuration files located at `~/.config/controller/config.toml` by default. Key configuration parameters include:
- Deadzone settings for joysticks and gyroscope
- Sensitivity settings for cursor movement and scrolling
- Reconnection settings for automatic device recovery
- Button mapping configurations

### Testing
Unit tests are integrated with the Rust testing framework and can be run with `cargo test`.

## Dependencies
- `hidapi` - HID device communication
- `enigo` - Cross-platform input simulation (using a custom fork)
- `serde` - Serialization/deserialization support
- `toml` - Configuration file format support
- `dirs` - System directory path retrieval