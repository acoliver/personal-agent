# PersonalAgent

A native macOS menu bar application for conversational AI. PersonalAgent lives in your system tray and provides quick access to various LLM providers through a clean, minimal interface.

## Features

- **Menu Bar App** - Click the icon in your macOS menu bar to open a chat panel
- **Multiple Providers** - Support for OpenAI, Anthropic, and OpenAI-compatible APIs (like Synthetic, GLM, etc.)
- **Model Profiles** - Create and switch between different model configurations
- **Streaming Responses** - Real-time streaming of AI responses
- **Thinking/Reasoning** - Display model thinking content for supported models (Claude, GLM-4, etc.)
- **Conversation History** - Persistent storage of chat conversations
- **Dark Theme** - Native dark mode UI that fits with macOS

## Installation

### From Source

```bash
git clone https://github.com/acoliver/personal-agent.git
cd personal-agent
cargo build --release
```

The binary will be at `target/release/personal_agent_menubar`.

### Running

```bash
./target/release/personal_agent_menubar
```

Look for "PA" in your menu bar. Click to open the chat panel.

## Configuration

Configuration is stored at `~/Library/Application Support/PersonalAgent/config.json`.

### Setting Up a Profile

1. Click the gear icon () to open settings
2. Click "+" to add a new profile
3. Select a provider and model from the models.dev registry, or configure manually
4. Enter your API key
5. Optionally configure a custom system prompt
6. Save the profile

### API Keys

API keys are read from the profile configuration. For security, you can also use keyfiles or environment variables depending on the provider.

## Usage

1. Click the PA icon in your menu bar
2. Select a profile from the dropdown (or use the default)
3. Type your message and press Enter or click Send
4. View streaming responses in real-time

### Keyboard Shortcuts

- `Cmd+V` - Paste text
- `Cmd+C` - Copy text
- `Cmd+A` - Select all
- `Cmd+Q` - Quit (from Edit menu)

### Thinking Mode

For models that support reasoning/thinking (like Claude with extended thinking or GLM-4):
- Enable "Thinking" in the profile settings
- Toggle the T* button to show/hide thinking content
- Thinking appears in a separate, dimmer bubble above responses

## Project Structure

```
personalAgent/
├── src/
│   ├── main_menubar.rs    # Application entry point
│   ├── lib.rs             # Library exports
│   ├── config/            # Configuration management
│   ├── models/            # Data models (profiles, conversations)
│   ├── storage/           # Conversation persistence
│   ├── llm/               # LLM client integration
│   ├── ui/                # Legacy UI components (deprecated)
│   └── ui_gpui/           # GPUI-based UI system (active development)
│       ├── bridge/        # Runtime bridge between GPUI and tokio
│       ├── components/    # Reusable UI components
│       ├── views/         # Main view components
│       ├── app.rs         # GPUI application
│       └── theme.rs       # Color theming
├── assets/                # Icons and images
└── research/
    └── serdesAI/          # LLM communication library
```

## GPUI-based UI System

PersonalAgent is transitioning to a GPUI-based UI system for better performance and native UI experience. The new UI system is currently implemented behind a feature flag.

### Feature Flag

Enable the GPUI UI system with:
```bash
cargo run --features gpui --bin personal_agent_menubar
```

### Architecture

The `ui_gpui` module implements a new UI architecture based on GPUI (smol-based) that communicates with the presenter/service layer (tokio-based) through a bridge pattern:

1. **Bridge Layer**: `GpuiBridge` and `ViewCommandSink` provide runtime communication
2. **Components**: Reusable UI components like `TabBar`, `MessageBubble`, `InputBar`, and `Button`
3. **Views**: Main view components including `ChatView`, `HistoryView`, `SettingsView`, and `MainPanel`
4. **Integration**: `TrayBridge`, `PopupWindow`, and `GpuiApp` integrate with system UI

Event flow follows: UserEvent → Bridge → EventBus → Presenter → ViewCommand → Bridge → UI

See `src/ui_gpui/` for more detailed documentation.

## Dependencies

- **objc2** - Rust bindings for macOS Cocoa APIs
- **serdes-ai** - LLM provider abstraction (local fork)
- **uuid** - Profile and conversation IDs
- **chrono** - Timestamps
- **serde** - Configuration serialization

## Development

```bash
# Build debug
cargo build --bin personal_agent_menubar

# Build release
cargo build --release --bin personal_agent_menubar

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy
```

### Debug Logging

Debug logs are written to `~/Library/Application Support/PersonalAgent/debug.log`.

## License

MIT

## Contributing

Contributions welcome! Please open an issue or PR on GitHub.
