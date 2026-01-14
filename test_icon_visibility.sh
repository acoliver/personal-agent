#!/bin/bash
# Test icon visibility by creating a high-contrast version

ICON_PATH="/Users/acoliver/projects/personalAgent/assets/MenuIcon.imageset/icon-32.png"
OUTPUT_PATH="/tmp/personal_agent_test_icons"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

mkdir -p "$OUTPUT_PATH"

echo -e "${BLUE}Icon Visibility Tester${NC}"
echo ""
echo "Original icon: $ICON_PATH"
echo ""

# Check if ImageMagick is available
if ! command -v convert &> /dev/null; then
    echo -e "${YELLOW}ImageMagick not found. Install with: brew install imagemagick${NC}"
    echo "Showing basic icon info instead:"
    echo ""
    sips -g all "$ICON_PATH"
    exit 0
fi

echo -e "${BLUE}Creating test variants...${NC}"
echo ""

# 1. High contrast black icon
echo "1. Creating high-contrast black icon..."
convert "$ICON_PATH" -colorspace gray -level 0%,50% "$OUTPUT_PATH/black_icon.png"
echo -e "${GREEN}   Created: $OUTPUT_PATH/black_icon.png${NC}"

# 2. High contrast white icon
echo "2. Creating high-contrast white icon..."
convert "$ICON_PATH" -negate "$OUTPUT_PATH/white_icon.png"
echo -e "${GREEN}   Created: $OUTPUT_PATH/white_icon.png${NC}"

# 3. Red background test icon
echo "3. Creating red background test icon..."
convert -size 32x32 xc:red "$OUTPUT_PATH/red_bg.png"
convert "$OUTPUT_PATH/red_bg.png" "$ICON_PATH" -composite "$OUTPUT_PATH/red_icon.png"
echo -e "${GREEN}   Created: $OUTPUT_PATH/red_icon.png${NC}"

# 4. Inverted original
echo "4. Creating inverted icon..."
convert "$ICON_PATH" -channel RGB -negate "$OUTPUT_PATH/inverted_icon.png"
echo -e "${GREEN}   Created: $OUTPUT_PATH/inverted_icon.png${NC}"

# 5. Scaled up for inspection
echo "5. Creating enlarged icon for inspection..."
convert "$ICON_PATH" -scale 256x256 "$OUTPUT_PATH/enlarged_icon.png"
echo -e "${GREEN}   Created: $OUTPUT_PATH/enlarged_icon.png${NC}"

echo ""
echo -e "${BLUE}Analysis:${NC}"
echo ""

# Analyze the original icon
echo "Original icon properties:"
identify -verbose "$ICON_PATH" | grep -E "Geometry|Colorspace|Alpha|mean:"

echo ""
echo -e "${YELLOW}Test these icons:${NC}"
echo "1. Copy a test icon over the original:"
echo "   cp $OUTPUT_PATH/black_icon.png assets/MenuIcon.imageset/icon-32.png"
echo ""
echo "2. Rebuild and run the app"
echo ""
echo "3. If you can see the test icon, the original is too subtle"
echo ""
echo "Available test icons:"
ls -lh "$OUTPUT_PATH"/*.png

echo ""
echo -e "${BLUE}To restore original:${NC}"
echo "git checkout assets/MenuIcon.imageset/icon-32.png"
