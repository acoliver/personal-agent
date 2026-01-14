# PersonalAgent - Phase 0

Minimal viable macOS menu bar application with tray icon and empty panel.

## Phase 0 Features

- [OK] Menu bar icon visible in macOS system tray
- [OK] Click icon to show/hide empty panel (400x500px)
- [OK] Dark theme background (#0d0d0d)
- [OK] Quit option in tray menu
- [OK] Quality gates: formatting, clippy, complexity checks, 80%+ test coverage

## Build & Run

```bash
cargo build --release
cargo run
```

## Run Tests

```bash
cargo test
```

## Quality Checks

```bash
./scripts/check-quality.sh
```

Checks:
- Code formatting (`cargo fmt`)
- Clippy lints (`cargo clippy`)
- Code complexity (lizard)
- Test coverage (>= 80%)

## Project Structure

```
personal-agent/
├── src/
│   └── main.rs          # Main application entry point
├── assets/
│   └── icon_32.png      # Tray icon (32x32px)
├── scripts/
│   └── check-quality.sh # Quality gate script
├── Cargo.toml           # Dependencies and lints
├── .clippy.toml         # Clippy configuration
├── .rustfmt.toml        # Rustfmt configuration
└── .git/hooks/
    └── pre-commit       # Pre-commit quality check
```

## Dependencies

- `eframe` - Native window management
- `egui` - Immediate-mode GUI framework
- `tray-icon` - System tray icon management
- `image` - Icon loading
- `tracing` - Logging

## Next Steps (Phase 1)

- Full dependency setup
- Configuration system
- Model profiles
- Conversation storage
- models.dev integration
