# Phase 05: Bridge Implementation

## Phase ID

`PLAN-20250128-GPUI.P05`

## Prerequisites

- Phase 04a completed with PASS
- Evidence file: `project-plans/gpui-migration/plan/.completed/P04A.md`
- Tests from Phase 04 exist and fail

---

## CRITICAL: Implementation Phase Rules

This is an **IMPLEMENTATION** phase. The following are **COMPLETE FAILURE**:

- `unimplemented!()` anywhere = **FAIL**
- `todo!()` anywhere = **FAIL**
- `// TODO` comments = **FAIL**
- Placeholder values = **FAIL**
- Empty function bodies = **FAIL**

**All Phase 04 tests must pass after this phase.**

---

## Requirements Implemented

### REQ-GPUI-006: Bridge Integration
- REQ-GPUI-006.1: flume channels for cross-runtime communication
- REQ-GPUI-006.2: UserEvent emission via try_send (non-blocking)
- REQ-GPUI-006.3: ViewCommand drain via try_recv loop
- REQ-GPUI-006.4: ViewCommandSink wraps sender + notifier
- REQ-GPUI-006.5: Notifier called after sending ViewCommand

---

## Implementation

### 1. `src/ui_gpui/bridge/gpui_bridge.rs`

```rust
//! GpuiBridge - main bridge struct for GPUI side
//!
//! @plan PLAN-20250128-GPUI.P05
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
/// @plan PLAN-20250128-GPUI.P05
/// @requirement REQ-GPUI-006.1
pub struct GpuiBridge {
    user_tx: Sender<UserEvent>,
    view_rx: Receiver<ViewCommand>,
}

impl GpuiBridge {
    /// Create a new bridge with the given channels
    ///
    /// @plan PLAN-20250128-GPUI.P05
    pub fn new(user_tx: Sender<UserEvent>, view_rx: Receiver<ViewCommand>) -> Self {
        Self { user_tx, view_rx }
    }

    /// Emit a UserEvent to the tokio runtime (non-blocking)
    ///
    /// Uses `try_send` to avoid blocking the GPUI thread.
    /// Returns true if sent, false if channel full or disconnected.
    ///
    /// @plan PLAN-20250128-GPUI.P05
    /// @requirement REQ-GPUI-006.2
    pub fn emit(&self, event: UserEvent) -> bool {
        match self.user_tx.try_send(event) {
            Ok(()) => true,
            Err(TrySendError::Full(evt)) => {
                tracing::warn!("UserEvent channel full, dropping: {:?}", evt);
                false
            }
            Err(TrySendError::Disconnected(evt)) => {
                tracing::warn!("UserEvent channel disconnected, dropping: {:?}", evt);
                false
            }
        }
    }

    /// Drain all pending ViewCommands (non-blocking)
    ///
    /// Uses `try_recv` in a loop to collect all available commands.
    /// Returns empty vec if no commands pending.
    ///
    /// @plan PLAN-20250128-GPUI.P05
    /// @requirement REQ-GPUI-006.3
    pub fn drain_commands(&self) -> Vec<ViewCommand> {
        let mut commands = Vec::new();
        while let Ok(cmd) = self.view_rx.try_recv() {
            commands.push(cmd);
        }
        commands
    }

    /// Check if there are pending ViewCommands
    ///
    /// @plan PLAN-20250128-GPUI.P05
    pub fn has_pending_commands(&self) -> bool {
        !self.view_rx.is_empty()
    }
}
```

### 2. `src/ui_gpui/bridge/view_command_sink.rs`

```rust
//! ViewCommandSink - wrapper for presenters to send ViewCommands
//!
//! @plan PLAN-20250128-GPUI.P05
//! @requirement REQ-GPUI-006.4
//! @requirement REQ-GPUI-006.5

use flume::{Sender, TrySendError};
use crate::presentation::ViewCommand;

/// Notifier handle to wake GPUI from tokio
///
/// @plan PLAN-20250128-GPUI.P05
pub trait GpuiNotifier: Send + Sync + Clone {
    /// Wake the GPUI thread to process ViewCommands
    fn notify(&self);
}

/// Sink for presenters to send ViewCommands to GPUI
///
/// Wraps a flume sender with a GPUI notifier.
/// After sending a command, calls notify() to wake GPUI.
///
/// @plan PLAN-20250128-GPUI.P05
/// @requirement REQ-GPUI-006.4
pub struct ViewCommandSink<N: GpuiNotifier> {
    tx: Sender<ViewCommand>,
    notifier: N,
}

impl<N: GpuiNotifier> ViewCommandSink<N> {
    /// Create a new sink with the given sender and notifier
    ///
    /// @plan PLAN-20250128-GPUI.P05
    pub fn new(tx: Sender<ViewCommand>, notifier: N) -> Self {
        Self { tx, notifier }
    }

    /// Send a ViewCommand and wake GPUI (non-blocking)
    ///
    /// Uses `try_send` to avoid blocking the tokio task.
    /// Always calls notify() to ensure GPUI wakes up.
    ///
    /// @plan PLAN-20250128-GPUI.P05
    /// @requirement REQ-GPUI-006.5
    pub fn send(&self, cmd: ViewCommand) {
        match self.tx.try_send(cmd) {
            Ok(()) => {
                self.notifier.notify();
            }
            Err(TrySendError::Full(cmd)) => {
                tracing::warn!("ViewCommand channel full, dropping: {:?}", cmd);
                // Still notify in case GPUI is behind on draining
                self.notifier.notify();
            }
            Err(TrySendError::Disconnected(cmd)) => {
                tracing::info!("ViewCommand channel disconnected (GPUI closed), dropping: {:?}", cmd);
                // No point notifying if disconnected
            }
        }
    }

    /// Clone the sender (for passing to multiple presenters)
    ///
    /// @plan PLAN-20250128-GPUI.P05
    pub fn clone_sender(&self) -> Sender<ViewCommand> {
        self.tx.clone()
    }
}

impl<N: GpuiNotifier> Clone for ViewCommandSink<N> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            notifier: self.notifier.clone(),
        }
    }
}
```

### 3. `src/ui_gpui/bridge/user_event_forwarder.rs`

```rust
//! UserEvent forwarder - tokio task that bridges flume to EventBus
//!
//! @plan PLAN-20250128-GPUI.P05
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
/// @plan PLAN-20250128-GPUI.P05
/// @requirement REQ-GPUI-006.1
pub fn spawn_user_event_forwarder(
    event_bus: Arc<EventBus>,
    user_rx: Receiver<UserEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("UserEvent forwarder started");
        
        while let Ok(event) = user_rx.recv_async().await {
            tracing::debug!("Forwarding UserEvent: {:?}", event);
            
            // Publish to EventBus (may fail if no subscribers, that's OK)
            if let Err(e) = event_bus.publish(AppEvent::User(event)) {
                tracing::debug!("EventBus publish failed (no subscribers?): {:?}", e);
            }
        }
        
        tracing::info!("UserEvent forwarder shutting down (sender dropped)");
    })
}
```

### 4. `src/ui_gpui/bridge/mod.rs`

```rust
//! Runtime bridge between GPUI (smol) and tokio
//!
//! This module provides the infrastructure for communication between
//! the GPUI UI (smol-based) and the presenter/service layer (tokio-based).
//!
//! ## Architecture
//!
//! ```text
//!     (GPUI/smol)                    (tokio)
//!   ┌─────────────┐              ┌─────────────┐
//!   │ GpuiBridge  │──UserEvent──►│ Forwarder   │──►EventBus
//!   │             │              │             │
//!   │             │◄─ViewCmd────│ViewCmdSink  │◄──Presenter
//!   └─────────────┘   +notify   └─────────────┘
//! ```
//!
//! @plan PLAN-20250128-GPUI.P05
//! @requirement REQ-GPUI-006

pub mod gpui_bridge;
pub mod view_command_sink;
pub mod user_event_forwarder;

pub use gpui_bridge::GpuiBridge;
pub use view_command_sink::{GpuiNotifier, ViewCommandSink};
pub use user_event_forwarder::spawn_user_event_forwarder;
```

---

## Verification Commands

### All Tests Pass

```bash
cargo test --test gpui_bridge_tests
```

Expected: All 12+ tests pass

### No Placeholders

```bash
grep -rn "unimplemented!" src/ui_gpui/bridge/
grep -rn "todo!" src/ui_gpui/bridge/
grep -rn "// TODO" src/ui_gpui/bridge/
```

Expected: NO MATCHES

### Build Succeeds

```bash
cargo build
```

Expected: Success

---

## Success Criteria

- [ ] All Phase 04 tests pass
- [ ] No placeholders in code
- [ ] `cargo build` succeeds
- [ ] GpuiBridge emit/drain work correctly
- [ ] ViewCommandSink sends and notifies
- [ ] UserEvent forwarder publishes to EventBus

---

## Evidence File

Create: `project-plans/gpui-migration/plan/.completed/P05.md`

```markdown
# Phase 05: Bridge Implementation Evidence

## Verdict: [PASS|FAIL]

## Test Results
```bash
$ cargo test --test gpui_bridge_tests 2>&1
[paste full output]
```

All tests pass: [YES/NO]

## Placeholder Detection
```bash
$ grep -rn "unimplemented!" src/ui_gpui/bridge/
[paste output - should be empty]

$ grep -rn "todo!" src/ui_gpui/bridge/
[paste output - should be empty]
```

No placeholders: [YES/NO]

## Build Result
```bash
$ cargo build 2>&1 | tail -5
[paste output]
```

Build succeeds: [YES/NO]

## Verdict Justification
[Explain]
```

---

## Next Phase

After P05 completes with PASS:
--> P05a: Bridge Implementation Verification
