#!/bin/bash
# Test script for personal-agent

cd "$(dirname "$0")"

echo "Building personal-agent..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

echo ""
echo "================================================================"
echo "Starting personal-agent..."
echo "================================================================"
echo ""
echo "The app will start and you should see a tray icon in your menu bar."
echo "Look for it in the top-right area of your screen."
echo ""
echo "What to expect:"
echo "  1. A small icon should appear in your menu bar"
echo "  2. Click the icon to show the popover"
echo "  3. Click again to hide it"
echo "  4. Right-click or secondary-click on the icon to see the menu"
echo "  5. Select 'Quit' from the menu to exit"
echo ""
echo "Press Ctrl+C in this terminal to stop the app if needed."
echo ""
echo "================================================================"
echo ""

RUST_LOG=info ./target/release/personal_agent

echo ""
echo "App terminated."
