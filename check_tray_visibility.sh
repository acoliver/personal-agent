#!/bin/bash
# Quick tray icon visibility checker
# Focuses on the most likely causes of invisible tray icons on macOS

set -e

PROJECT_DIR="/Users/acoliver/projects/personalAgent/personal-agent"
BINARY_NAME="personal_agent"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Quick Tray Icon Visibility Check${NC}"
echo ""

# Check 1: Icon file visibility
echo -e "${BLUE}1. Checking icon file properties...${NC}"
ICON_PATH="$PROJECT_DIR/../assets/MenuIcon.imageset/icon-32.png"

if [ -f "$ICON_PATH" ]; then
    echo -e "${GREEN}[OK]${NC} Icon file exists"
    
    # Get image info using sips (built-in macOS tool)
    echo "   Image info:"
    sips -g all "$ICON_PATH" | grep -E "pixelWidth|pixelHeight|format|hasAlpha"
    
    # Check if image has alpha channel (transparency)
    if sips -g hasAlpha "$ICON_PATH" | grep -q "yes"; then
        echo -e "${GREEN}[OK]${NC} Icon has alpha channel (supports transparency)"
    else
        echo -e "${YELLOW}${NC} Icon lacks alpha channel"
    fi
    
    # Create a visible test icon (red background) for debugging
    echo ""
    echo "Creating high-visibility test icon..."
    TEST_ICON="/tmp/personal_agent_test_icon.png"
    
    # Use ImageMagick if available, otherwise skip
    if command -v convert &> /dev/null; then
        convert -size 32x32 xc:red -fill white -draw "circle 16,16 16,8" "$TEST_ICON"
        echo -e "${GREEN}[OK]${NC} Created test icon: $TEST_ICON"
        echo "   (Red background with white dot - very visible)"
    else
        echo -e "${YELLOW}${NC} ImageMagick not installed, skipping test icon creation"
        echo "   Install with: brew install imagemagick"
    fi
else
    echo -e "${RED}${NC} Icon file NOT FOUND: $ICON_PATH"
fi

echo ""
echo -e "${BLUE}2. Checking template mode setting...${NC}"
if grep -q "with_icon_as_template(true)" "$PROJECT_DIR/src/main.rs"; then
    echo -e "${YELLOW}${NC} Template mode is ENABLED"
    echo "   Template icons are monochrome and may be hard to see"
    echo "   Consider temporarily disabling for testing"
else
    echo -e "${GREEN}[OK]${NC} Template mode is disabled (icon should be full color)"
fi

echo ""
echo -e "${BLUE}3. Checking process status...${NC}"
if pgrep -x "$BINARY_NAME" > /dev/null; then
    PID=$(pgrep -x "$BINARY_NAME")
    echo -e "${GREEN}[OK]${NC} Process is running (PID: $PID)"
    
    # Check CPU usage (hung process will use 0% or 100%)
    CPU=$(ps -p $PID -o %cpu= | tr -d ' ')
    echo "   CPU usage: ${CPU}%"
    
    if (( $(echo "$CPU < 1" | bc -l) )); then
        echo -e "${GREEN}[OK]${NC} Process is idle (normal for menu bar app)"
    elif (( $(echo "$CPU > 50" | bc -l) )); then
        echo -e "${YELLOW}${NC} Process using high CPU (may indicate event loop issue)"
    fi
else
    echo -e "${YELLOW}${NC} Process is NOT running"
fi

echo ""
echo -e "${BLUE}4. Checking macOS menu bar status...${NC}"

# Check if menu bar is functional
if defaults read com.apple.menuextra.clock &> /dev/null; then
    echo -e "${GREEN}[OK]${NC} macOS menu bar preferences accessible"
else
    echo -e "${RED}${NC} Cannot access menu bar preferences"
fi

# Check display server
if pgrep -x "WindowServer" > /dev/null; then
    echo -e "${GREEN}[OK]${NC} WindowServer is running"
else
    echo -e "${RED}${NC} WindowServer not running (serious display issue!)"
fi

echo ""
echo -e "${BLUE}5. Analyzing recent logs...${NC}"

LOG_FILE="/tmp/personal_agent_debug.log"
if [ -f "$LOG_FILE" ]; then
    echo "Last log entries:"
    tail -10 "$LOG_FILE" | sed 's/^/   /'
    
    echo ""
    if grep -q "Tray icon created successfully" "$LOG_FILE"; then
        echo -e "${GREEN}[OK]${NC} Tray icon creation logged"
    else
        echo -e "${RED}${NC} No tray icon creation logged"
    fi
    
    if grep -q "Failed to create tray icon" "$LOG_FILE"; then
        echo -e "${RED}${NC} Tray icon creation FAILED"
        grep "Failed to create tray icon" "$LOG_FILE" | tail -1 | sed 's/^/   /'
    fi
else
    echo -e "${YELLOW}${NC} No log file found at $LOG_FILE"
    echo "   Run the app with: RUST_LOG=trace ./target/release/personal_agent"
fi

echo ""
echo -e "${BLUE}6. NSStatusItem debugging...${NC}"

# Check if we can query the process's objective-c objects
if command -v lldb &> /dev/null && pgrep -x "$BINARY_NAME" > /dev/null; then
    PID=$(pgrep -x "$BINARY_NAME")
    echo "Attempting to inspect NSStatusItem (may require sudo)..."
    
    # This is advanced debugging - create an lldb script
    cat > /tmp/lldb_inspect.txt << 'EOF'
expr @import Foundation
expr @import AppKit
expr (void)NSLog(@"Status items: %@", [[NSStatusBar systemStatusBar] statusItems])
quit
EOF
    
    echo "Note: This requires developer tools and may prompt for permission"
    echo "   Skipping automatic execution - run manually if needed:"
    echo "   sudo lldb -p $PID -s /tmp/lldb_inspect.txt"
else
    echo -e "${YELLOW}${NC} lldb not available or process not running"
fi

echo ""
echo -e "${BLUE}=== DIAGNOSIS SUMMARY ===${NC}"
echo ""

# Check the most likely issues
ISSUES_FOUND=0

if ! grep -q "Tray icon created successfully" "$LOG_FILE" 2>/dev/null; then
    echo -e "${RED} Issue 1: Tray icon creation may have failed${NC}"
    echo "   → Check the build logs for compilation errors"
    echo "   → Verify tray-icon crate is compatible with your macOS version"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

if grep -q "with_icon_as_template(true)" "$PROJECT_DIR/src/main.rs"; then
    echo -e "${YELLOW} Issue 2: Template mode makes icon subtle${NC}"
    echo "   → Try disabling template mode temporarily:"
    echo "   → Change .with_icon_as_template(true) to (false) in main.rs"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

if [ $ISSUES_FOUND -eq 0 ]; then
    echo -e "${GREEN}[OK] No obvious issues detected${NC}"
    echo ""
    echo "If icon still not visible, try:"
    echo "  1. Restart the app"
    echo "  2. Check System Settings > Privacy & Security"
    echo "  3. Run the comprehensive debug script: ./debug_tray_icon.sh"
else
    echo ""
    echo -e "${YELLOW}Found $ISSUES_FOUND potential issue(s)${NC}"
    echo "Run ./debug_tray_icon.sh for comprehensive testing"
fi

echo ""
