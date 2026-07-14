#!/usr/bin/env python3
"""Drive issue #151 selection behavior in the real Linux GPUI window."""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys
import time

MARKER = "ISSUE151_ALPHA"
WINDOW_ID = ""
WINDOW_X = 0
WINDOW_Y = 0


def run(*args: str, check: bool = True, capture: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, check=check, text=True, capture_output=capture)


def xdo(*args: str) -> None:
    run("xdotool", *args)


def clipboard() -> str:
    try:
        result = subprocess.run(
            ["xclip", "-selection", "clipboard", "-o"],
            check=False,
            text=True,
            capture_output=True,
            timeout=0.25,
        )
    except subprocess.TimeoutExpired:
        return ""
    return result.stdout


def set_clipboard(text: str) -> None:
    try:
        subprocess.run(
            ["xclip", "-selection", "clipboard", "-i"],
            input=text,
            text=True,
            check=True,
            timeout=1.0,
        )
    except subprocess.TimeoutExpired:
        pass


def geometry(window_id: str) -> dict[str, int]:
    values: dict[str, int] = {}
    for line in run("xdotool", "getwindowgeometry", "--shell", window_id).stdout.splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            if key in {"X", "Y", "WIDTH", "HEIGHT"}:
                values[key] = int(value)
    return values


def screenshot(window_id: str, path: pathlib.Path) -> None:
    run("import", "-window", window_id, str(path))


def move_local(x: int, y: int) -> None:
    xdo("mousemove", str(WINDOW_X + x), str(WINDOW_Y + y))


def image_changed_pixels(before: pathlib.Path, after: pathlib.Path) -> int:
    metric = run(
        "compare", "-metric", "AE", str(before), str(after), "null:", check=False
    ).stderr.strip()
    return int(float((metric or "0").split()[0]))


def drag(
    start_x: int,
    end_x: int,
    y: int,
    held_path: pathlib.Path | None = None,
    baseline_path: pathlib.Path | None = None,
) -> float | None:
    move_local(start_x, y)
    xdo("mousedown", "1")
    latency: float | None = None
    try:
        time.sleep(0.03)
        moved_at = time.monotonic()
        for step in range(1, 7):
            x = start_x + (end_x - start_x) * step // 6
            move_local(x, y)
            time.sleep(0.005)
        if held_path is not None and baseline_path is not None:
            deadline = moved_at + 12.0
            while time.monotonic() < deadline:
                screenshot(WINDOW_ID, held_path)
                if image_changed_pixels(baseline_path, held_path) > 0:
                    latency = time.monotonic() - moved_at
                    break
                time.sleep(0.02)
    finally:
        xdo("mouseup", "1")
        time.sleep(0.03)
    return latency


def copy_selection(window_id: str) -> str:
    xdo("windowactivate", "--sync", window_id)
    xdo("key", "--window", window_id, "ctrl+c")
    time.sleep(0.08)
    return clipboard()


def main() -> int:
    if len(sys.argv) != 4:
        print("usage: issue151_selection_linux.py WINDOW_ID ARTIFACT_DIR LOG", file=sys.stderr)
        return 2

    global WINDOW_ID, WINDOW_X, WINDOW_Y
    window_id, artifact_dir_text, log_path_text = sys.argv[1:]
    WINDOW_ID = window_id
    artifact_dir = pathlib.Path(artifact_dir_text)
    log_path = pathlib.Path(log_path_text)
    artifact_dir.mkdir(parents=True, exist_ok=True)
    before_path = artifact_dir / "before-selection.png"
    selected_path = artifact_dir / "selected.png"
    held_selection_path = artifact_dir / "selection-while-mouse-held.png"
    menu_path = artifact_dir / "context-menu.png"
    composer_forward_path = artifact_dir / "composer-forward-selection.png"
    composer_reverse_path = artifact_dir / "composer-reverse-selection.png"

    xdo("windowactivate", "--sync", window_id)
    geo = geometry(window_id)
    WINDOW_X = geo["X"]
    WINDOW_Y = geo["Y"]
    width = geo["WIDTH"]
    height = geo["HEIGHT"]
    screenshot(window_id, before_path)

    # Scan the transcript using broad horizontal drags. The fixture contains the
    # marker in both user and assistant bubbles, so a copied marker proves the
    # shipped message rendering path accepted a real pointer drag.
    found: tuple[int, int, int, str] | None = None
    # The fixture and test popup have deterministic geometry. Try exact rich
    # heading/user-bubble rows first, then fall back to a denser grid so font or
    # platform metric changes do not make the runner brittle.
    candidates = [
        (24, min(width - 20, 380), 192),
        (max(24, width - 300), width - 40, 108),
        (24, min(width - 20, 360), 224),
    ]
    for y in (100, 108, 116, 184, 192, 200, 216, 224, 232, 248, 272, 296):
        for start_x in (16, 24, 40, max(20, width - 320), max(20, width - 300)):
            candidates.append((start_x, min(width - 20, start_x + 400), y))
            candidates.append((min(width - 20, start_x + 280), start_x, y))

    for candidate_index, (start_x, end_x, y) in enumerate(candidates):
        set_clipboard("ISSUE151_NO_SELECTION")
        drag(start_x, end_x, y)
        copied = copy_selection(window_id)
        if candidate_index < 3:
            screenshot(window_id, artifact_dir / f"candidate-{candidate_index}.png")
            print(f"candidate {candidate_index}: ({start_x},{y})->({end_x},{y}) copied={copied!r}")
        if MARKER in copied:
            found = (start_x, end_x, y, copied)
            break

    if found is None:
        print(f"FAIL: no selectable {MARKER} text found", file=sys.stderr)
        print(log_path.read_text(errors="replace")[-4000:], file=sys.stderr)
        return 1

    start_x, end_x, y, copied = found
    # Repeat the successful gesture and capture before mouse-up. This proves
    # that selection feedback is live rather than appearing only on release.
    move_local(12, height - 80)
    xdo("click", "1")
    time.sleep(0.03)
    held_selection_latency = drag(
        start_x,
        end_x,
        y,
        held_selection_path,
        before_path,
    )
    screenshot(window_id, selected_path)

    held_changed_pixels = image_changed_pixels(before_path, held_selection_path)
    if held_changed_pixels <= 0 or held_selection_latency is None:
        print("FAIL: no selection pixels changed within 12s while mouse was held", file=sys.stderr)
        return 1
    if held_selection_latency > 0.75:
        print(
            f"FAIL: held selection took {held_selection_latency:.3f}s to paint",
            file=sys.stderr,
        )
        return 1

    # A changed image is machine-verifiable evidence that selection produced a
    # visible highlight, not merely a hidden range and successful clipboard.
    metric = run(
        "compare", "-metric", "AE", str(before_path), str(selected_path), "null:",
        check=False,
    ).stderr.strip()
    changed_pixels = int(float((metric or "0").split()[0]))
    if changed_pixels <= 0:
        print("FAIL: selection screenshot has no changed pixels", file=sys.stderr)
        return 1

    # Right-click within the selected row must show the real root-level menu.
    menu_x = (start_x + end_x) // 2
    move_local(menu_x, y)
    xdo("click", "3")
    time.sleep(0.15)
    screenshot(window_id, menu_path)
    menu_metric = run(
        "compare", "-metric", "AE", str(selected_path), str(menu_path), "null:",
        check=False,
    ).stderr.strip()
    menu_changed_pixels = int(float((menu_metric or "0").split()[0]))
    if menu_changed_pixels <= 0:
        print("FAIL: context-menu screenshot has no changed pixels", file=sys.stderr)
        return 1

    # Click the Copy item near the menu origin and prove it uses the immutable
    # context-menu selection snapshot.
    set_clipboard("ISSUE151_CONTEXT_MENU_NOT_USED")
    move_local(min(width - 20, menu_x + 25), min(height - 20, y + 16))
    xdo("click", "1")
    time.sleep(0.1)
    menu_copy = clipboard()
    if MARKER not in menu_copy:
        print(f"FAIL: context-menu Copy returned {menu_copy!r}", file=sys.stderr)
        return 1

    # Exercise the shipped composer element with real X11 pointer and keyboard
    # events. Selection ownership must move from the transcript to the composer.
    composer_text = "ISSUE151_COMPOSER alpha beta café"
    input_y = height - 31
    input_start_x = 24
    input_end_x = min(width - 100, 230)
    move_local(input_start_x, input_y)
    xdo("click", "1")
    xdo("key", "ctrl+a")
    set_clipboard(composer_text)
    xdo("key", "ctrl+v")
    time.sleep(0.08)

    # Select-all and copy proves keyboard selection and clipboard routing.
    xdo("key", "ctrl+a")
    selected_all = copy_selection(window_id)
    if selected_all != composer_text:
        print(f"FAIL: composer select-all copied {selected_all!r}", file=sys.stderr)
        return 1

    # Forward mouse drag must produce a proper partial selection.
    time.sleep(0.5)
    drag(input_start_x, input_end_x, input_y)
    composer_forward = copy_selection(window_id)
    screenshot(window_id, composer_forward_path)
    if not composer_forward or composer_forward == composer_text or composer_forward not in composer_text:
        print(f"FAIL: composer forward drag copied {composer_forward!r}", file=sys.stderr)
        return 1

    # Reverse drag must select the same visible region without a warm-up click.
    xdo("key", "ctrl+a")
    set_clipboard(composer_text)
    xdo("key", "ctrl+v")
    time.sleep(0.5)
    drag(input_end_x, input_start_x, input_y)
    composer_reverse = copy_selection(window_id)
    screenshot(window_id, composer_reverse_path)
    if not composer_reverse or composer_reverse == composer_text or composer_reverse not in composer_text:
        print(f"FAIL: composer reverse drag copied {composer_reverse!r}", file=sys.stderr)
        return 1

    # Cut must remove the pointer-selected range. Read the remaining text via
    # select-all so the assertion is independent of rendering details.
    xdo("key", "ctrl+x")
    time.sleep(0.05)
    xdo("key", "ctrl+a")
    composer_after_cut = copy_selection(window_id)
    if len(composer_after_cut) >= len(composer_text):
        print(f"FAIL: composer cut left {composer_after_cut!r}", file=sys.stderr)
        return 1

    # Typing while selected replaces that selection; Unicode paste remains
    # intact through the real clipboard and input-handler path.
    replacement = "ISSUE151_REPLACED café"
    set_clipboard(replacement)
    xdo("key", "ctrl+v")
    time.sleep(0.08)
    xdo("key", "ctrl+a")
    composer_replaced = copy_selection(window_id)
    if composer_replaced != replacement:
        print(f"FAIL: composer replacement produced {composer_replaced!r}", file=sys.stderr)
        return 1


    evidence = artifact_dir / "evidence.txt"
    evidence.write_text(
        "\n".join(
            [
                "Issue #151 Linux production-window E2E: PASS",
                f"window={window_id} geometry={geo}",
                f"drag=({start_x},{y})->({end_x},{y})",
                f"keyboard_copy={copied!r}",
                f"context_menu_copy={menu_copy!r}",
                f"composer_select_all={selected_all!r}",
                f"composer_forward_drag={composer_forward!r}",
                f"composer_reverse_drag={composer_reverse!r}",
                f"composer_after_cut={composer_after_cut!r}",
                f"composer_replaced={composer_replaced!r}",
                f"selection_while_held_changed_pixels={held_changed_pixels}",
                f"held_selection_latency_ms={held_selection_latency * 1000:.1f}",
                f"selection_changed_pixels={changed_pixels}",
                f"menu_changed_pixels={menu_changed_pixels}",
            ]
        )
        + "\n"
    )
    print(evidence.read_text(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
