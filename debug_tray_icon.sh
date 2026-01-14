#!/bin/bash
# Comprehensive tray icon debug script for personal-agent
# This script will help diagnose why the tray icon isn't appearing in macOS menu bar

set -e

PROJECT_DIR="/Users/acoliver/projects/personalAgent/personal-agent"
BINARY_NAME="personal_agent"
LOG_FILE="/tmp/personal_agent_debug.log"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE}Personal Agent Tray Icon Debug Script${NC}"
echo -e "${BLUE}================================================${NC}"
echo ""

# Function to print colored messages
info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Step 1: Kill any existing processes
info "Step 1: Cleaning up existing processes..."
if pkill -9 "$BINARY_NAME" 2>/dev/null; then
    success "Killed existing $BINARY_NAME processes"
    sleep 1
else
    info "No existing processes found"
fi

# Step 2: Check icon files
info "Step 2: Verifying icon files exist..."
ICON_PATH="$PROJECT_DIR/../assets/MenuIcon.imageset/icon-32.png"
if [ -f "$ICON_PATH" ]; then
    success "Icon file found: $ICON_PATH"
    FILE_SIZE=$(wc -c < "$ICON_PATH")
    info "Icon file size: $FILE_SIZE bytes"
    
    # Check if it's a valid PNG
    if file "$ICON_PATH" | grep -q "PNG"; then
        success "Icon is a valid PNG file"
    else
        error "Icon file is not a valid PNG!"
    fi
else
    error "Icon file NOT FOUND: $ICON_PATH"
fi

# Step 3: Build the app with verbose output
info "Step 3: Building the app (release mode)..."
cd "$PROJECT_DIR"
if cargo build --release 2>&1 | tee "$LOG_FILE.build"; then
    success "Build completed successfully"
else
    error "Build failed! Check $LOG_FILE.build for details"
    exit 1
fi

# Step 4: Check macOS permissions
info "Step 4: Checking macOS permissions..."
info "If the tray icon doesn't appear, you may need to grant permissions in System Settings > Privacy & Security"

# Step 5: Create a test version with non-template icon
info "Step 5: Creating test version with non-template icon (more visible)..."
info "Modifying main.rs temporarily to disable template mode..."

# Backup original main.rs
cp "$PROJECT_DIR/src/main.rs" "$PROJECT_DIR/src/main.rs.backup"

# Modify the template setting
sed -i '' 's/.with_icon_as_template(true)/.with_icon_as_template(false)/' "$PROJECT_DIR/src/main.rs"

info "Building non-template version..."
if cargo build --release 2>&1 | tee "$LOG_FILE.build2"; then
    success "Non-template build completed"
else
    error "Non-template build failed!"
    # Restore original
    mv "$PROJECT_DIR/src/main.rs.backup" "$PROJECT_DIR/src/main.rs"
    exit 1
fi

# Step 6: Run with maximum logging
info "Step 6: Running app with RUST_LOG=trace..."
info "The app will run in the background. Watch for tray icon in menu bar."
echo ""
warning "==> LOOK FOR THE TRAY ICON IN YOUR MENU BAR NOW! <==" 
echo ""

# Clear old log
> "$LOG_FILE"

# Run the app in background with trace logging
RUST_LOG=trace "$PROJECT_DIR/target/release/$BINARY_NAME" >> "$LOG_FILE" 2>&1 &
APP_PID=$!

success "App started with PID: $APP_PID"
info "Logs being written to: $LOG_FILE"
echo ""

# Step 7: Monitor for tray icon creation
info "Step 7: Monitoring logs for tray icon creation (5 seconds)..."
sleep 1

for i in {1..5}; do
    echo -n "."
    sleep 1
done
echo ""

# Check logs for key messages
info "Checking logs..."
if grep -q "Tray icon created successfully" "$LOG_FILE"; then
    success "Found: 'Tray icon created successfully'"
else
    error "Did NOT find: 'Tray icon created successfully'"
fi

if grep -q "Popover initialized successfully" "$LOG_FILE"; then
    success "Found: 'Popover initialized successfully'"
else
    warning "Did NOT find: 'Popover initialized successfully'"
fi

if grep -q "Failed" "$LOG_FILE"; then
    warning "Found failure messages in logs:"
    grep "Failed" "$LOG_FILE" | head -5
fi

if grep -q "Error" "$LOG_FILE"; then
    warning "Found error messages in logs:"
    grep "Error" "$LOG_FILE" | head -5
fi

echo ""
info "Recent log output:"
echo "---"
tail -20 "$LOG_FILE"
echo "---"
echo ""

# Step 8: Interactive test
info "Step 8: Interactive visibility test"
echo ""
echo -e "${YELLOW}=== VISIBILITY TEST ===${NC}"
echo "Please check your macOS menu bar (top right area) for:"
echo "  1. A small icon near the system clock/battery/wifi icons"
echo "  2. The icon should be a small image (32x32 pixels)"
echo ""
echo -n "Can you see the tray icon? (y/n): "
read -r RESPONSE

if [[ "$RESPONSE" =~ ^[Yy]$ ]]; then
    success "Great! The tray icon is visible!"
    echo ""
    echo -n "Try clicking the icon. Did a popover/window appear? (y/n): "
    read -r CLICK_RESPONSE
    
    if [[ "$CLICK_RESPONSE" =~ ^[Yy]$ ]]; then
        success "Perfect! The app is working correctly!"
        echo ""
        info "Checking logs for click event..."
        sleep 1
        if grep -q "Tray click event received" "$LOG_FILE"; then
            success "Click event was logged!"
        else
            warning "Click event not found in logs - check $LOG_FILE"
        fi
    else
        error "Icon visible but clicks not working"
        info "This suggests an event handling issue"
    fi
else
    error "Tray icon is NOT visible"
    echo ""
    info "Possible causes:"
    echo "  1. Icon is too subtle (try non-template mode - this test uses it)"
    echo "  2. macOS permissions not granted"
    echo "  3. Icon creation silently failed"
    echo "  4. NSStatusItem not properly added to menu bar"
    echo ""
    info "Let's try some diagnostics..."
    
    # Check if process is running
    if ps -p $APP_PID > /dev/null; then
        success "Process is still running (PID: $APP_PID)"
    else
        error "Process has died!"
        info "Check logs: $LOG_FILE"
    fi
    
    # Check system.log for relevant messages
    info "Checking system logs for permission issues..."
    if log show --predicate 'process == "personal_agent"' --last 1m 2>/dev/null | grep -i "permission\|deny\|sandbox" | head -5; then
        warning "Found permission-related messages in system log"
    fi
fi

echo ""
echo -e "${BLUE}=== NEXT STEPS ===${NC}"
echo ""

# Step 9: Restore original main.rs
info "Step 9: Restoring original main.rs..."
mv "$PROJECT_DIR/src/main.rs.backup" "$PROJECT_DIR/src/main.rs"
success "Original main.rs restored"

echo ""
info "Summary of files:"
echo "  - Build logs: $LOG_FILE.build and $LOG_FILE.build2"
echo "  - Runtime logs: $LOG_FILE"
echo "  - App PID: $APP_PID"
echo ""

echo -n "Do you want to keep the app running? (y/n): "
read -r KEEP_RESPONSE

if [[ ! "$KEEP_RESPONSE" =~ ^[Yy]$ ]]; then
    info "Stopping the app..."
    kill $APP_PID 2>/dev/null || true
    success "App stopped"
else
    info "App is still running with PID: $APP_PID"
    info "To stop it: kill $APP_PID"
    info "To view logs: tail -f $LOG_FILE"
fi

echo ""
echo -e "${BLUE}=== RECOMMENDATIONS ===${NC}"
echo ""
echo "If the icon is NOT visible:"
echo "  1. Check System Settings > Privacy & Security for any personal_agent permissions"
echo "  2. Try running: defaults read com.apple.menuextra.clock IsAnalog"
echo "     (This verifies menu bar is working)"
echo "  3. Check if other menu bar apps work"
echo "  4. Review the full logs in: $LOG_FILE"
echo ""
echo "If the icon IS visible but clicks don't work:"
echo "  1. Review popover.rs event handling"
echo "  2. Check for NSStatusItem button creation"
echo "  3. Verify TrayIconEvent::receiver() is working"
echo ""
echo -e "${GREEN}Debug script complete!${NC}"
