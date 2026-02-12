# GPUI UI Architecture

## Overview

The GPUI-based UI system provides a modern, high-performance interface for PersonalAgent using the GPUI framework. This architecture replaces the legacy Cococa-based UI system currently implemented in the `ui` module.

## Module Structure

### ui_gpui/

The `ui_gpui` module is the top-level module for the GPUI-based UI system. It exports the main components needed by the rest of the application.

#### Exports
- `GpuiApp`: Main GPUI application
- `GpuiBridge`: Runtime bridge between GPUI and tokio
- `ViewCommandSink`: Sink for sending view commands to GPUI
- `spawn_user_event_forwarder`: Function to spawn event forwarding task
- `PopupWindow`: Popup window implementation
- `TrayBridge`: Bridge for tray integration

### bridge/

The bridge layer provides the infrastructure for communication between the GPUI UI (smol-based) and the presenter/service layer (tokio-based).

#### Components

**GpuiBridge**
- Primary bridge between GPUI and tokio
- Handles bidirectional communication
- Manages the event forwarding pipeline
- Coordinates with `ViewCommandSink` for UI updates

**ViewCommandSink**
- Receives view commands from the presenter layer
- Converts them into UI updates through GPUI
- Notifies the GPUI runtime of state changes

**user_event_forwarder**
- Spawns a background task to forward user events
- Bridges GPUI user events to the tokio EventBus
- Handles async event processing

#### Architecture Diagram

```
    (GPUI/smol)                    (tokio)
  ┌─────────────┐              ┌─────────────┐
  │ GpuiBridge  │──UserEvent──►│ Forwarder   │──►EventBus
  │             │              │             │
  │             │◄─ViewCmd────│ViewCmdSink  │◄──Presenter
  └─────────────┘   +notify   └─────────────┘
```

### components/

Reusable UI components that can be used across different views.

#### Components

**TabBar**
- Tab navigation component
- Manages tab selection and rendering
- Supports custom tab configurations

**MessageBubble**
- Renders message content (user and assistant)
- Handles different message types (text, thinking)
- Supports markdown rendering

**InputBar**
- Text input component with send button
- Handles keyboard shortcuts (Enter to send)
- Manages input state and focus

**Button**
- Generic button component
- Supports different styles and states
- Handles click events and actions

### views/

Main view components that compose the application interface.

#### Views

**ChatView**
- Main chat interface with message history
- Manages message list rendering
- Handles input flow and message sending

**MainPanel**
- Root container with tab navigation
- Manages view switching (chat, history, settings)
- Handles layout and sizing

**HistoryView**
- Displays conversation history
- Supports search and filtering
- Manages conversation selection and deletion

**SettingsView**
- Configuration interface
- Manages profiles and settings
- Handles form inputs and validation

### Integration

**TrayBridge**
- Integration with system tray
- Handles tray icon and menu
- Manages popup window lifecycle

**PopupWindow**
- Popup window for the chat interface
- Handles positioning and visibility
- Manages focus and window events

**GpuiApp**
- Main GPUI application entry point
- Initializes the UI context
- Sets up the runtime and event loop
- Manages application lifecycle

## Event Flow

1. **User Interaction**: User interacts with UI components
2. **UserEvent Creation**: Component creates a `UserEvent` instance
3. **Bridge Forwarding**: `GpuiBridge` forwards the event through `user_event_forwarder`
4. **EventBus Dispatch**: Event is dispatched on the tokio EventBus
5. **Presenter Processing**: Presenter receives event and processes business logic
6. **ViewCommand Generation**: Presenter generates `ViewCommand` instances
7. **Bridge Delivery**: `ViewCommandSink` delivers commands to GPUI
8. **UI Update**: Components update their state and re-render

## Usage

### Creating a New UI Component

1. Implement the `Component` trait from GPUI
2. Add the component to the `components` module
3. Export it from `components/mod.rs`
4. Use the component in views

### Adding a New View

1. Create view implementation in `views/` directory
2. Implement view-specific state and event handling
3. Export the view from `views/mod.rs`
4. Integrate with `MainPanel` if needed

### Extending Event Handling

1. Add new `UserEvent` variants to the event enum
2. Add handling in the appropriate presenter
3. Ensure bidirectional communication is properly set up

## Implementation Details

- The bridge layer uses async channels for communication
- All UI operations happen on the GPUI (smol) thread
- Business logic remains in the tokio domain
- State management follows a unidirectional flow pattern
- Components are designed to be reusable and composable

## Migration Notes

This architecture is being phased in to replace the legacy Cocoa-based UI. Both systems coexist during the transition period. The GPUI system is enabled with the `gpui` feature flag.