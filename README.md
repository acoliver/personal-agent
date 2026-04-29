# Personal Agent

Personal Agent is a native desktop app for chatting with AI models from your menu bar or system tray. It keeps a compact chat panel one click away, lets you switch between model profiles, streams responses as they arrive, and stores API keys in your operating system credential store.

![Personal Agent chat panel](assets/screenshots/personal-agent-main.png)

## What it does

- Opens from the macOS menu bar, Windows system tray, or Linux StatusNotifierItem/AppIndicator area.
- Chats with OpenAI, Anthropic, and OpenAI-compatible APIs.
- Saves reusable model profiles so you can switch providers or models quickly.
- Streams responses in real time and preserves conversation history.
- Supports model thinking/reasoning output when the selected model exposes it.
- Stores API keys securely in Keychain Services, Windows Credential Manager, or Secret Service.
- Runs as a native Rust/GPUI app with a dark desktop UI.

## Install

### macOS with Homebrew

Personal Agent is published through the `acoliver/homebrew-tap` Homebrew tap:

```bash
brew tap acoliver/homebrew-tap
brew install personal-agent
```

Launch it after installation and look for the Personal Agent icon in your menu bar.

### macOS from a release

Download the latest macOS artifact from [GitHub Releases](https://github.com/acoliver/personal-agent/releases), extract it, and open the app.

### Windows

Download the latest Windows ZIP from [GitHub Releases](https://github.com/acoliver/personal-agent/releases), extract it, and run `personal-agent.exe`.

Windows Defender SmartScreen may warn about unsigned binaries. If you trust the release, choose **More info** and **Run anyway**.

### Linux

Download the latest Linux package from [GitHub Releases](https://github.com/acoliver/personal-agent/releases):

- `.deb` for Debian/Ubuntu
- `.rpm` for Fedora/RHEL
- `.zip` for a portable archive

The Linux tray uses the StatusNotifierItem protocol. KDE Plasma supports it out of the box. GNOME users may need an AppIndicator/SNI extension.

## First run

1. Open Personal Agent from the menu bar or tray.
2. Click the gear icon to open settings.
3. Add or edit a model profile.
4. Choose a provider/model, enter your API key, and save the profile.
5. Select the profile in the chat panel and send a message.

For a complete setup guide, including a Z.ai GLM-5.1 coding profile example, see [docs/walkthrough.md](docs/walkthrough.md).

## Where settings are stored

- macOS: `~/Library/Application Support/PersonalAgent/`
- Windows: `%LOCALAPPDATA%\PersonalAgent\`
- Linux: `${XDG_CONFIG_HOME:-~/.config}/PersonalAgent/` and `${XDG_DATA_HOME:-~/.local/share}/PersonalAgent/`

API key values are stored in the OS credential store, not directly in profile JSON files.

## Build from source

Install Rust, then run:

```bash
git clone https://github.com/acoliver/personal-agent.git
cd personal-agent
cargo build --release --bin personal_agent_gpui
```

The built binary is `target/release/personal_agent_gpui` on macOS/Linux and `target\release\personal_agent_gpui.exe` on Windows.

## Development

```bash
cargo build --bin personal_agent_gpui
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --lib --tests
cargo qa
```

The project uses a Rust-native `xtask` runner for `cargo qa` and `cargo coverage`.

## License

MIT

## Contributing

Contributions are welcome. Please open an issue or pull request on GitHub.
