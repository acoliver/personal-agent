# PersonalAgent

A native desktop application for conversational AI. PersonalAgent lives in your system tray and provides quick access to various LLM providers through a clean, minimal interface.

## Features

- **System Tray App** - Click the icon in your menu bar (macOS), system tray (Windows), or StatusNotifierItem area (Linux) to open a chat panel
- **Multiple Providers** - Support for OpenAI, Anthropic, and OpenAI-compatible APIs (like Synthetic, GLM, etc.)
- **Model Profiles** - Create and switch between different model configurations
- **Streaming Responses** - Real-time streaming of AI responses
- **Thinking/Reasoning** - Display model thinking content for supported models (Claude, GLM-4, etc.)
- **Conversation History** - Persistent storage of chat conversations
- **Dark Theme** - Native dark mode UI that fits with your desktop environment
- **Cross-Platform** - Native support for macOS, Windows, and Linux

## Installation

### macOS

#### From Release (Recommended)

Download the latest release from [GitHub Releases](https://github.com/acoliver/personal-agent/releases) and extract the `.app` bundle.

#### From Homebrew

```bash
brew tap acoliver/tap
brew install personal-agent
```

#### From Source

```bash
git clone https://github.com/acoliver/personal-agent.git
cd personal-agent
cargo build --release
```

The binary will be at `target/release/personal_agent_gpui`.

### Windows

#### From Release (Recommended)

Download the latest ZIP archive from [GitHub Releases](https://github.com/acoliver/personal-agent/releases):
1. Download `personal-agent-vX.Y.Z-x86_64-pc-windows-msvc.zip`
2. Extract the archive to your preferred location
3. Run `personal-agent.exe`

> **Note:** Windows Defender may show a SmartScreen warning for unsigned binaries. Click "More info" and "Run anyway" to proceed.

#### From Source

```powershell
git clone https://github.com/acoliver/personal-agent.git
cd personal-agent
cargo build --release --bin personal_agent_gpui
```

The executable will be at `target\release\personal_agent_gpui.exe`.

### Linux

#### From Release

Download the appropriate package from [GitHub Releases](https://github.com/acoliver/personal-agent/releases):
- `.deb` for Debian/Ubuntu
- `.rpm` for Fedora/RHEL
- `.zip` portable archive for any distribution

#### From Source

PersonalAgent supports Linux builds (Wayland-first) with SNI tray integration.
Before building on Debian/Ubuntu-based distributions, install the required packages:

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential \
  clang \
  cmake \
  curl \
  gcc \
  g++ \
  git \
  jq \
  libdbus-1-dev \
  libasound2-dev \
  libfontconfig-dev \
  libgit2-dev \
  libglib2.0-dev \
  libgtk-3-dev \
  libssl-dev \
  libva-dev \
  libvulkan1 \
  libwayland-dev \
  libx11-xcb-dev \
  libxkbcommon-x11-dev \
  libzstd-dev \
  llvm \
  lld \
  make \
  pkg-config \
  pipewire \
  xdg-desktop-portal
```

The Linux tray uses the StatusNotifierItem protocol. KDE Plasma supports this out-of-the-box.
On GNOME, install/enable an AppIndicator/SNI extension to make tray icons visible.

## Running

```bash
./target/release/personal_agent_gpui
```

- **macOS**: Look for the PA icon in your menu bar. Click to open the chat panel.
- **Windows**: Look for the PersonalAgent icon in the system tray (notification area). Click to open the chat panel, or right-click for a context menu.
- **Linux**: Look for the PersonalAgent tray icon in your desktop environment's StatusNotifierItem/AppIndicator area.

## Configuration

### macOS

Configuration is stored at `~/Library/Application Support/PersonalAgent/config.json`.

### Windows

Configuration is stored at `%LOCALAPPDATA%\PersonalAgent\config.json`.

### Linux

Configuration is stored at `${XDG_CONFIG_HOME:-~/.config}/personal-agent/config.json`.

### Setting Up a Profile

1. Click the gear icon to open settings
2. Click "+" to add a new profile
3. Select a provider and model from the models.dev registry, or configure manually
4. Enter your API key
5. Optionally configure a custom system prompt
6. Save the profile

### API Keys

API keys are securely stored in your OS credential store:
- **macOS**: Keychain Services
- **Windows**: Credential Manager
- **Linux**: Secret Service (GNOME Keyring / KDE Wallet)

## Usage

1. Click the PersonalAgent icon in your system tray/menu bar
2. Select a profile from the dropdown (or use the default)
3. Type your message and press Enter or click Send
4. View streaming responses in real-time

### Keyboard Shortcuts

- `Ctrl/Cmd+V` - Paste text
- `Ctrl/Cmd+C` - Copy text
- `Ctrl/Cmd+A` - Select all
- `Ctrl/Cmd+Q` - Quit (from Edit menu)

### Thinking Mode

For models that support reasoning/thinking (like Claude with extended thinking or GLM-4):
- Enable "Thinking" in the profile settings
- Toggle the T* button to show/hide thinking content
- Thinking appears in a separate, dimmer bubble above responses

## Project Structure

```
personalAgent/
├── src/
│   ├── main_gpui.rs       # Application entry point
│   ├── lib.rs             # Library exports
│   ├── config/            # Configuration management
│   ├── models/            # Data models (profiles, conversations)
│   ├── storage/           # Conversation persistence
│   ├── llm/               # LLM client integration
│   └── ui_gpui/           # GPUI-based UI system
│       ├── bridge/        # Runtime bridge between GPUI and tokio
│       ├── components/    # Reusable UI components
│       ├── views/         # Main view components
│       ├── app.rs         # GPUI application
│       └── theme.rs       # Color theming
├── assets/                # Icons and images
└── research/
    └── serdesAI/          # LLM communication library
```


## UI Architecture

PersonalAgent uses a GPUI-based UI system for native performance and a modern UI experience.



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
cargo build --bin personal_agent_gpui

# Build release
cargo build --release --bin personal_agent_gpui

# Run tests
cargo test

# Check formatting
cargo fmt --all -- --check

# Run clippy
cargo clippy --all-targets -- -D warnings

# Run the full local quality gate
cargo qa

# Run just the coverage gate
cargo coverage
```

The project includes a small Rust-native `xtask` runner rather than relying on a large shell script.
`cargo qa` runs formatting, clippy, tests, and coverage using Cargo subcommands.
Coverage reports still write build artifacts under `target/llvm-cov-target`, which is Cargo's normal output area rather than `src/bin`.
`src/bin/` contains additional binary source files, not compiled output artifacts.

### Debug Logging

- **macOS**: `~/Library/Application Support/PersonalAgent/debug.log`
- **Windows**: `%LOCALAPPDATA%\PersonalAgent\debug.log`
- **Linux**: `${XDG_CONFIG_HOME:-~/.config}/personal-agent/debug.log`

## License

MIT

## Contributing

Contributions welcome! Please open an issue or PR on GitHub.
