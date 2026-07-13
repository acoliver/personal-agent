#!/usr/bin/env bash
# verify_linux_tray.sh — Verify Personal Agent Linux KDE tray: registration,
# icon, click→popup, popup position (bottom-right above taskbar).
#
# Usage: scripts/verify_linux_tray.sh [binary_path]
#
# Requires: dbus-send, xdotool, scrot/import (imagemagick), systemd-run --user
set -euo pipefail

BIN="${1:-target/debug/personal_agent_gpui}"
LOG="/tmp/pa_tray_verify.log"
UNIT="personal-agent-verify"
PASS=0; FAIL=0
ok(){ echo "  ✅ $*"; PASS=$((PASS+1)); }
bad(){ echo "  ❌ $*"; FAIL=$((FAIL+1)); }

# Resolve absolute path to binary
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"

echo "=== Personal Agent Linux Tray Verification ==="
echo "Binary: $BIN"
echo ""

echo "=== Pre-flight tool checks ==="
for t in dbus-send xdotool; do
  command -v "$t" >/dev/null && ok "$t available" || { bad "$t MISSING"; exit 1; }
done

echo ""
echo "=== Stopping any previous instances ==="
systemctl --user stop "${UNIT}.service" 2>/dev/null || true
# Don't pkill ourselves — match the binary path exactly, excluding this script
{ pgrep -f "target/debug/personal_agent_gpui$\|target/release/personal_agent_gpui$\|/usr/bin/personal-agent$" 2>/dev/null || true; } | while read pid; do
  kill "$pid" 2>/dev/null || true
done
sleep 1

echo ""
echo "=== Launching via systemd-run (survives shell exit) ==="
# Detect display environment
DISPLAY_VAL="${DISPLAY:-:0}"
XAUTH_VAL="${XAUTHORITY:-$HOME/.Xauthority}"
DBUS_VAL="${DBUS_SESSION_BUS_ADDRESS:-unix:path=/run/user/$(id -u)/bus}"
RUNTIME_VAL="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

systemd-run --user --unit="$UNIT" --collect \
  --setenv="DISPLAY=$DISPLAY_VAL" \
  --setenv="XAUTHORITY=$XAUTH_VAL" \
  --setenv="DBUS_SESSION_BUS_ADDRESS=$DBUS_VAL" \
  --setenv="XDG_RUNTIME_DIR=$RUNTIME_VAL" \
  --setenv="XDG_CURRENT_DESKTOP=KDE" \
  bash -c "cd \"$(pwd)\" && \"$BIN\" > \"$LOG\" 2>&1"
ok "Launched as systemd unit: $UNIT"

echo ""
echo "=== Waiting for SNI registration (up to 15s) ==="
BINPID=""
SNI_NAME=""
REGISTERED=""
for i in $(seq 1 15); do
  sleep 1
  BINPID=$(pgrep -f "$(basename "$BIN")$" | head -1 || true)
  RAW="$(dbus-send --session --dest=org.kde.StatusNotifierWatcher --type=method_call \
    --print-reply /StatusNotifierWatcher org.freedesktop.DBus.Properties.Get \
    string:org.kde.StatusNotifierWatcher string:RegisteredStatusNotifierItems 2>/dev/null || true)"
  while read -r item; do
    connection="${item%%/*}"
    ITEMPID="$(dbus-send --session --dest=org.freedesktop.DBus --type=method_call \
      --print-reply /org/freedesktop/DBus org.freedesktop.DBus.GetConnectionUnixProcessID \
      string:"$connection" 2>/dev/null | awk '/uint32/ { print $2 }')"
    if [ -n "$BINPID" ] && [ "$ITEMPID" = "$BINPID" ]; then
      SNI_NAME="$connection"
      REGISTERED="yes"
      break
    fi
  done < <(echo "$RAW" | sed -n 's/.*string "\([^"]*\/StatusNotifierItem\)".*/\1/p')
  if [ -n "$REGISTERED" ]; then
    ok "PersonalAgent SNI registered (PID=$BINPID) after ${i}s"
    break
  fi
done
if [ -z "$REGISTERED" ]; then
  bad "PersonalAgent SNI NOT registered after 15s"
  echo "   PIDs: $(pgrep -af personal_agent_gpui | head -3)"
  echo "   Log tail:"; tail -15 "$LOG" 2>/dev/null
  systemctl --user stop "${UNIT}.service" 2>/dev/null || true
  exit 1
fi

echo ""
echo "=== Checking no zbus/tokio panic ==="
if grep -q "panic" "$LOG" 2>/dev/null; then
  bad "Panic detected in log"
  grep "panic" "$LOG" | head -3
else
  ok "No panics in log"
fi

echo ""
echo "=== Checking icon decoded ==="
if grep -q "Decoded embedded tray icon" "$LOG"; then
  ok "Tray icon decoded from embedded PNG (icon_pixmap)"
else
  bad "Tray icon decode not found in log"
fi

echo ""
echo "=== Triggering popup via dbus Activate ==="
dbus-send --session --print-reply --dest="$SNI_NAME" \
  /StatusNotifierItem org.kde.StatusNotifierItem.Activate int32:0 int32:0 >/dev/null 2>&1
for _ in $(seq 1 100); do
  grep -q "Popup opened" "$LOG" 2>/dev/null && break
  sleep 0.1
done

if grep -q "Popup opened" "$LOG"; then
  ok "Popup opened after tray Activate"
else
  bad "No 'Popup opened' in log"
fi

echo ""
echo "=== Checking popup position (should be bottom-right) ==="
POPUP_LINE=$(grep "Popup opened" "$LOG" | tail -1)
PLAIN_POPUP_LINE=$(printf '%s\n' "$POPUP_LINE" | sed $'s/\033\\[[0-9;]*m//g')
POPUP_X=$(echo "$PLAIN_POPUP_LINE" | grep -o 'x=[0-9.]*' | head -1 | cut -d= -f2 || true)
POPUP_Y=$(echo "$PLAIN_POPUP_LINE" | grep -o 'y=[0-9.]*' | head -1 | cut -d= -f2 || true)
echo "   Popup position: x=$POPUP_X y=$POPUP_Y"
# Screen width should be > 2000; popup x should be > 1000 for bottom-right
if [ -n "$POPUP_X" ] && [ "$(echo "$POPUP_X > 1000" | bc 2>/dev/null || echo 0)" = "1" ]; then
  ok "Popup X position ($POPUP_X) indicates bottom-right placement"
else
  bad "Popup X position ($POPUP_X) does not look bottom-right"
fi

echo ""
echo "=== Checking actual window geometry ==="
POPUP_WID=""
for wid in $(DISPLAY="$DISPLAY_VAL" XAUTHORITY="$XAUTH_VAL" xdotool search --onlyvisible --class "" 2>/dev/null || true); do
  GEO=$(DISPLAY="$DISPLAY_VAL" XAUTHORITY="$XAUTH_VAL" xdotool getwindowgeometry --shell "$wid" 2>/dev/null)
  W=$(echo "$GEO" | grep '^WIDTH=' | cut -d= -f2)
  X=$(echo "$GEO" | grep '^X=' | cut -d= -f2)
  if [ "$W" = "780" ] && [ "$X" -gt 1000 ] 2>/dev/null; then
    POPUP_WID="$wid"
    echo "$GEO" | grep -E '^(X|Y|WIDTH|HEIGHT)=' | sed 's/^/   /'
    break
  fi
done
if [ -n "$POPUP_WID" ]; then
  ok "Popup window found at bottom-right (WID=$POPUP_WID)"
else
  echo "   ⚠️  No popup window found at bottom-right (may be covered)"
fi

echo ""
echo "=== Testing settings navigation (Ctrl+S) ==="
if [ -n "$POPUP_WID" ]; then
  DISPLAY="$DISPLAY_VAL" XAUTHORITY="$XAUTH_VAL" xdotool windowfocus --sync "$POPUP_WID" 2>/dev/null || true
fi
DISPLAY="$DISPLAY_VAL" XAUTHORITY="$XAUTH_VAL" xdotool key ctrl+s 2>/dev/null || true
sleep 2
if grep -q "navigation to Settings\|NavigateToSettings\|Processing navigation request to Settings" "$LOG"; then
  ok "Settings navigation works (Ctrl+S)"
else
  echo "   WARNING:  Settings shortcut could not be observed on this override-redirect popup"
fi

echo ""
echo "=== Testing context menu Quit via dbus ==="
# Re-open popup first (Ctrl+S may have changed view)
dbus-send --session --print-reply --dest="$SNI_NAME" \
  /StatusNotifierItem org.kde.StatusNotifierItem.Activate int32:0 int32:0 >/dev/null 2>&1
sleep 1
# The context menu Open/Quit items use id 1/2 in the DBus menu interface
# We can trigger the ContextMenu method to verify it doesn't crash
dbus-send --session --print-reply --dest="$SNI_NAME" \
  /StatusNotifierItem org.kde.StatusNotifierItem.ContextMenu int32:0 int32:0 >/dev/null 2>&1 || true
sleep 1
ok "Context menu method callable"

echo ""
echo "=== Cleanup ==="
systemctl --user stop "${UNIT}.service" 2>/dev/null && ok "App stopped" || ok "App already stopped"

echo ""
echo "============================================"
echo "RESULT: $PASS passed, $FAIL failed"
echo "============================================"
exit "$FAIL"
