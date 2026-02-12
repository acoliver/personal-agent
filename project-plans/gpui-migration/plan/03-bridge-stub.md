# Phase 03: Bridge Stub

## Phase ID

`PLAN-20250128-GPUI.P03`

## Prerequisites

- Phase 02a completed with PASS
- Evidence file: `project-plans/gpui-migration/plan/.completed/P02A.md`

---

## Purpose

Create the runtime bridge module that enables communication between GPUI (smol) and tokio runtimes using `flume` channels.

**This is the critical integration point.** Get this wrong and nothing else works.

---

## Requirements Implemented

### REQ-GPUI-006: Bridge Integration
- REQ-GPUI-006.1: Use `flume` channels for cross-runtime communication
- REQ-GPUI-006.2: GPUI emits UserEvents via `try_send()` (non-blocking)
- REQ-GPUI-006.3: GPUI receives ViewCommands via `try_recv()` drain loop

---

## Files to Create

### 1. `src/ui_gpui/bridge/mod.rs`

```rust
//! Runtime bridge between GPUI (smol) and tokio
//!
//! @plan PLAN-20250128-GPUI.P03
//! @requirement REQ-GPUI-006

pub mod gpui_bridge;
pub mod view_command_sink;
pub mod user_event_forwarder;

pub use gpui_bridge::GpuiBridge;
pub use view_command_sink::ViewCommandSink;
pub use user_event_forwarder::spawn_user_event_forwarder;
```

### 2. `src/ui_gpui/bridge/gpui_bridge.rs`

```rust
//! GpuiBridge - main bridge struct for GPUI side
//!
//! @plan PLAN-20250128-GPUI.P03
//! @requirement REQ-GPUI-006.1
//! @requirement REQ-GPUI-006.2
//! @requirement REQ-GPUI-006.3

use flume::{Receiver, Sender, TrySendError};
use crate::events::types::UserEvent;
use crate::presentation::ViewCommand;

/// Bridge between GPUI (smol) and tokio runtimes
///
/// Owned by the GPUI main panel. Provides non-blocking
/// send/receive for cross-runtime communication.
///
/// @plan PLAN-20250128-GPUI.P03
/// @requirement REQ-GPUI-006.1
pub struct GpuiBridge {
    /// Send UserEvents from GPUI to tokio (-> EventBus)
    user_tx: Sender<UserEvent>,
    /// Receive ViewCommands from tokio presenters
    view_rx: Receiver<ViewCommand>,
}

impl GpuiBridge {
    /// Create a new bridge with the given channels
    ///
    /// @plan PLAN-20250128-GPUI.P03
    pub fn new(user_tx: Sender<UserEvent>, view_rx: Receiver<ViewCommand>) -> Self {
        unimplemented!("Phase 05: GpuiBridge::new")
    }

    /// Emit a UserEvent to the tokio runtime (non-blocking)
    ///
    /// Uses `try_send` to avoid blocking the GPUI thread.
    /// Returns true if sent, false if channel full or disconnected.
    ///
    /// @plan PLAN-20250128-GPUI.P03
    /// @requirement REQ-GPUI-006.2
    pub fn emit(&self, event: UserEvent) -> bool {
        unimplemented!("Phase 05: GpuiBridge::emit")
    }

    /// Drain all pending ViewCommands (non-blocking)
    ///
    /// Uses `try_recv` in a loop to collect all available commands.
    /// Returns empty vec if no commands pending.
    ///
    /// @plan PLAN-20250128-GPUI.P03
    /// @requirement REQ-GPUI-006.3
    pub fn drain_commands(&self) -> Vec<ViewCommand> {
        unimplemented!("Phase 05: GpuiBridge::drain_commands")
    }

    /// Check if there are pending ViewCommands
    ///
    /// @plan PLAN-20250128-GPUI.P03
    pub fn has_pending_commands(&self) -> bool {
        unimplemented!("Phase 05: GpuiBridge::has_pending_commands")
    }
}
```

### 3. `src/ui_gpui/bridge/view_command_sink.rs`

```rust
//! ViewCommandSink - wrapper for presenters to send ViewCommands
//!
//! @plan PLAN-20250128-GPUI.P03
//! @requirement REQ-GPUI-006.4
//! @requirement REQ-GPUI-006.5

use flume::Sender;
use crate::presentation::ViewCommand;

/// Notifier handle to wake GPUI from tokio
///
/// This is a placeholder type - will be replaced with actual
/// GPUI notifier handle type in implementation phase.
///
/// @plan PLAN-20250128-GPUI.P03
pub trait GpuiNotifier: Send + Sync + Clone {
    /// Wake the GPUI thread to process ViewCommands
    fn notify(&self);
}

/// Sink for presenters to send ViewCommands to GPUI
///
/// Wraps a flume sender with a GPUI notifier.
/// After sending a command, calls notify() to wake GPUI.
///
/// @plan PLAN-20250128-GPUI.P03
/// @requirement REQ-GPUI-006.4
pub struct ViewCommandSink<N: GpuiNotifier> {
    tx: Sender<ViewCommand>,
    notifier: N,
}

impl<N: GpuiNotifier> ViewCommandSink<N> {
    /// Create a new sink with the given sender and notifier
    ///
    /// @plan PLAN-20250128-GPUI.P03
    pub fn new(tx: Sender<ViewCommand>, notifier: N) -> Self {
        unimplemented!("Phase 05: ViewCommandSink::new")
    }

    /// Send a ViewCommand and wake GPUI (non-blocking)
    ///
    /// Uses `try_send` to avoid blocking the tokio task.
    /// Always calls notify() to ensure GPUI wakes up.
    ///
    /// @plan PLAN-20250128-GPUI.P03
    /// @requirement REQ-GPUI-006.5
    pub fn send(&self, cmd: ViewCommand) {
        unimplemented!("Phase 05: ViewCommandSink::send")
    }

    /// Clone the sender (for passing to multiple presenters)
    ///
    /// @plan PLAN-20250128-GPUI.P03
    pub fn clone_sender(&self) -> Sender<ViewCommand> {
        unimplemented!("Phase 05: ViewCommandSink::clone_sender")
    }
}

impl<N: GpuiNotifier> Clone for ViewCommandSink<N> {
    fn clone(&self) -> Self {
        unimplemented!("Phase 05: ViewCommandSink::clone")
    }
}
```

### 4. `src/ui_gpui/bridge/user_event_forwarder.rs`

```rust
//! UserEvent forwarder - tokio task that bridges flume to EventBus
//!
//! @plan PLAN-20250128-GPUI.P03
//! @requirement REQ-GPUI-006.1

use std::sync::Arc;
use flume::Receiver;
use tokio::task::JoinHandle;
use crate::events::{AppEvent, EventBus};
use crate::events::types::UserEvent;

/// Spawn a tokio task that forwards UserEvents from flume to EventBus
///
/// This task runs on the tokio runtime and:
/// 1. Receives UserEvents from the flume channel (from GPUI)
/// 2. Wraps them in AppEvent::User
/// 3. Publishes to the EventBus
///
/// The task exits when the sender is dropped (GPUI closed).
///
/// @plan PLAN-20250128-GPUI.P03
/// @requirement REQ-GPUI-006.1
pub fn spawn_user_event_forwarder(
    event_bus: Arc<EventBus>,
    user_rx: Receiver<UserEvent>,
) -> JoinHandle<()> {
    unimplemented!("Phase 05: spawn_user_event_forwarder")
}
```

### 5. Update `src/ui_gpui/mod.rs`

```rust
//! GPUI-based UI module for PersonalAgent
//!
//! @plan PLAN-20250128-GPUI.P03
//! @requirement REQ-GPUI-001

pub mod app;
pub mod theme;
pub mod tray;
pub mod bridge;
pub mod components;
pub mod views;
pub mod state;

pub use app::GpuiApp;
pub use theme::Theme;
pub use bridge::{GpuiBridge, ViewCommandSink, spawn_user_event_forwarder};
```

### 6. Update `Cargo.toml`

Add the `flume` dependency:

```toml
[dependencies]
# ... existing deps ...

# Runtime-agnostic channels for GPUI/tokio bridge
flume = "0.11"
```

---

## Verification Commands

### Files Exist

```bash
ls -la src/ui_gpui/bridge/
# Expected: mod.rs, gpui_bridge.rs, view_command_sink.rs, user_event_forwarder.rs
```

### Markers Present

```bash
grep -r "@plan PLAN-20250128-GPUI.P03" src/ui_gpui/bridge/
# Expected: 10+ occurrences

grep -r "@requirement REQ-GPUI-006" src/ui_gpui/bridge/
# Expected: 6+ occurrences
```

### Compiles

```bash
cargo build
# Expected: Success (stubs with unimplemented! are OK in stub phase)
```

### flume Dependency Added

```bash
grep "flume" Cargo.toml
# Expected: flume = "0.11"
```

---

## Success Criteria

- [ ] All bridge files created
- [ ] All files have @plan markers
- [ ] Requirement markers present (REQ-GPUI-006.*)
- [ ] `flume` dependency added to Cargo.toml
- [ ] `cargo build` succeeds
- [ ] Bridge module exported from `src/ui_gpui/mod.rs`

---

## Evidence File

Create: `project-plans/gpui-migration/plan/.completed/P03.md`

```markdown
# Phase 03: Bridge Stub Evidence

## Files Created
- [ ] src/ui_gpui/bridge/mod.rs
- [ ] src/ui_gpui/bridge/gpui_bridge.rs
- [ ] src/ui_gpui/bridge/view_command_sink.rs
- [ ] src/ui_gpui/bridge/user_event_forwarder.rs

## Marker Counts
```bash
$ grep -c "@plan PLAN-20250128-GPUI.P03" src/ui_gpui/bridge/*.rs
[paste output]
```

## Build Result
```bash
$ cargo build 2>&1 | tail -10
[paste output]
```

## flume Dependency
```bash
$ grep "flume" Cargo.toml
[paste output]
```

## Verdict: [PASS|FAIL]
```

---

## Next Phase

After P03 completes with PASS:
--> P03a: Bridge Stub Verification
