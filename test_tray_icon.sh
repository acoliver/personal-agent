#!/bin/bash

# Test script for PersonalAgent tray icon

echo "=================================================="
echo "PersonalAgent Tray Icon Test"
echo "=================================================="
echo ""

# Kill any existing instances
echo "1. Cleaning up existing instances..."
pkill -f personal_agent
sleep 1

# Build and launch
echo "2. Building release binary..."
cd /Users/acoliver/projects/personalAgent/personal-agent
cargo build --release --quiet

echo "3. Launching PersonalAgent..."
./target/release/personal_agent > /tmp/personal_agent.log 2>&1 &
APP_PID=$!
sleep 2

# Check if running
if ps -p $APP_PID > /dev/null; then
    echo "✅ App is running (PID: $APP_PID)"
else
    echo "❌ App failed to start"
    exit 1
fi

# Show logs
echo ""
echo "4. Application logs:"
echo "-------------------"
cat /tmp/personal_agent.log
echo ""

# Instructions
echo "=================================================="
echo "MANUAL TESTING INSTRUCTIONS:"
echo "=================================================="
echo ""
echo "👀 Look at your macOS menu bar (top-right corner, near the clock)"
echo ""
echo "You should see:"
echo "  • A small icon (may be subtle/monochrome due to template mode)"
echo "  • Icon should be visible among WiFi, battery, clock, etc."
echo ""
echo "To test the popover:"
echo "  1. Click the tray icon"
echo "  2. A popover should appear below it"
echo "  3. The popover should have a native arrow pointing at the icon"
echo "  4. Inside should show 'PersonalAgent' heading"
echo "  5. Dark background (RGB: 13, 13, 13)"
echo ""
echo "To close the popover:"
echo "  • Click the icon again"
echo "  • Or click outside the popover"
echo ""
echo "To quit the app:"
echo "  • Right-click the icon and select 'Quit'"
echo "  • Or run: pkill -f personal_agent"
echo ""
echo "=================================================="
echo "App is running. Press Ctrl+C to view logs in tail mode."
echo "=================================================="

# Tail logs
tail -f /tmp/personal_agent.log
