#!/bin/bash
# Run debug build of personal-agent with enhanced visibility testing

set -e

PROJECT_DIR="/Users/acoliver/projects/personalAgent/personal-agent"
BINARY_NAME="personal_agent"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Personal Agent Debug Build Runner ===${NC}"
echo ""

# Step 1: Kill existing processes
echo -e "${BLUE}Cleaning up existing processes...${NC}"
if pkill -9 "$BINARY_NAME" 2>/dev/null; then
    echo -e "${GREEN}Killed existing processes${NC}"
    sleep 1
fi

# Step 2: Backup and swap main.rs
echo -e "${BLUE}Setting up debug version...${NC}"
cd "$PROJECT_DIR"

if [ -f "src/main.rs.original" ]; then
    echo "Found existing backup, using it"
else
    cp "src/main.rs" "src/main.rs.original"
    echo "Backed up original main.rs"
fi

cp "src/main_debug.rs" "src/main.rs"
echo -e "${GREEN}Debug main.rs activated${NC}"

# Step 3: Build
echo ""
echo -e "${BLUE}Building debug version...${NC}"
if cargo build --release; then
    echo -e "${GREEN}Build successful!${NC}"
else
    echo -e "${RED}Build failed!${NC}"
    # Restore original
    cp "src/main.rs.original" "src/main.rs"
    exit 1
fi

# Step 4: Run with logging
echo ""
echo -e "${BLUE}Starting app with RUST_LOG=trace...${NC}"
echo ""
echo -e "${YELLOW}================================================${NC}"
echo -e "${YELLOW}LOOK FOR THE TRAY ICON IN YOUR MENU BAR${NC}"
echo -e "${YELLOW}(Top right corner, near clock/battery icons)${NC}"
echo -e "${YELLOW}================================================${NC}"
echo ""

LOG_FILE="/tmp/personal_agent_debug.log"
> "$LOG_FILE"

echo "Logs will be written to: $LOG_FILE"
echo ""
echo "Starting app..."

# Run in foreground with trace logging
RUST_LOG=trace "$PROJECT_DIR/target/release/$BINARY_NAME" 2>&1 | tee "$LOG_FILE"

# This will only execute after the app is closed
echo ""
echo -e "${BLUE}App closed, restoring original main.rs...${NC}"
cp "src/main.rs.original" "src/main.rs"
echo -e "${GREEN}Original main.rs restored${NC}"
