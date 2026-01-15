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
│   └── ui/                # UI components
│       ├── chat_view.rs   # Main chat interface
│       ├── settings_view.rs
│       ├── profile_editor*.rs
│       └── theme.rs       # Color theme
├── assets/                # Icons and images
└── research/
    └── serdesAI/          # LLM communication library
```

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
