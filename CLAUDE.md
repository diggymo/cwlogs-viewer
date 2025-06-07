# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

Build and run the application:
```bash
cargo run
```

Build for release:
```bash
cargo build --release
```

Run with debug logging:
```bash
export CWLOGS_VIEWER_LOG_LEVEL=debug
export CWLOGS_VIEWER_DATA=`pwd`/.data
cargo run
```

Check for clippy warnings:
```bash
cargo clippy
```

## Architecture

This is a TUI (Terminal User Interface) application for viewing Amazon CloudWatch Logs in real-time. The architecture follows a component-based design with async message passing:

- **Main Application Loop**: `app.rs` contains the core `App` struct that manages the event loop, action handling, and component lifecycle
- **Component System**: All UI components implement the `Component` trait (`components.rs`) which provides standardized event handling, rendering, and state management
- **Action-Based Communication**: Components communicate through an action system (`action.rs`) using `tokio::sync::mpsc` channels
- **AWS Integration**: Uses AWS SDK for CloudWatch Logs with live tail streaming functionality in `outer_layout.rs`

### Key Components

- `OuterLayout`: Main layout component that manages `LogGroupList` and `LogStream` components, handles AWS live tail streaming
- `LogGroupList`: Displays and manages selection of CloudWatch log groups
- `LogStream`: Displays real-time log messages from selected log groups

### AWS Configuration

The application expects AWS credentials to be configured via environment variables or AWS profiles. It connects to the `ap-northeast-1` region by default (hardcoded in `outer_layout.rs`).

### Environment Variables

- `CWLOGS_VIEWER_LOG_LEVEL`: Set logging level (debug, info, warn, error)
- `CWLOGS_VIEWER_DATA`: Directory for application data storage