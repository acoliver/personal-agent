# NSStackView Layout Fix - Summary

## Problem
The UI views were collapsing to zero height when using NSStackView. The popover showed as essentially blank/invisible.

## Root Cause
NSStackView relies on **intrinsic content size** of subviews. Views without intrinsic size (like NSScrollView) collapse to 0 height unless properly configured.

## Solution Applied

### 1. Added Required Cargo Features
**File:** `Cargo.toml`
- Added `NSStackView` feature to enable NSStackView bindings
- Added `NSLayoutConstraint` feature to enable layout priority APIs

### 2. Imported Required Types
**Files:** All UI view files (`chat_view.rs`, `history_view.rs`, `settings_view.rs`, `profile_editor.rs`)
- Added `NSStackViewDistribution` import
- Added `NSLayoutPriority` import (for chat_view.rs)

### 3. Set Distribution Property on Main Stack Views
**Critical Change:** Set `distribution` to `Fill` on all main vertical stack views
```rust
main_stack.setDistribution(NSStackViewDistribution::Fill);
```

This tells the stack view to expand its subviews to fill available space.

Applied to:
- `src/ui/chat_view.rs` - main stack
- `src/ui/history_view.rs` - main stack
- `src/ui/settings_view.rs` - main stack
- `src/ui/profile_editor.rs` - main stack

### 4. Set Content Hugging Priorities (chat_view.rs)
**Critical Change:** Set different priorities for different sections:
- **Top bar:** High priority (251.0) - wants to stay at fixed height (48px)
- **Chat area (scroll view):** Low priority (1.0) - wants to expand to fill remaining space
- **Input area:** High priority (251.0) - wants to stay at fixed height (50px)

```rust
// Top bar: high priority (wants to stay at fixed height)
top_bar.setContentHuggingPriority_forOrientation(
    251.0,
    NSLayoutConstraintOrientation::Vertical
);

// Chat area (scroll view): low priority (wants to expand)
chat_area.setContentHuggingPriority_forOrientation(
    1.0,
    NSLayoutConstraintOrientation::Vertical
);

// Input area: high priority (wants to stay at fixed height)
input_area.setContentHuggingPriority_forOrientation(
    251.0,
    NSLayoutConstraintOrientation::Vertical
);
```

### 5. Added Minimum Height Constraint to Scroll View
**Critical Change:** Added a minimum height constraint to prevent collapse:
```rust
let min_height = scroll_view.heightAnchor().constraintGreaterThanOrEqualToConstant(100.0);
min_height.setActive(true);
```

This ensures the scroll view always has at least 100px height, preventing it from collapsing to zero.

### 6. Added Debug Logging
Added frame debugging to verify layout after changes:
```rust
println!("\n=== ChatViewController Frame Debug ===");
println!("  main_view: {:?}", main_view.frame());
println!("  main_stack: {:?}", main_stack.frame());
println!("  top_bar: {:?}", top_bar.frame());
println!("  chat_area: {:?}", chat_area.frame());
println!("  input_area: {:?}", input_area.frame());
println!("=====================================\n");
```

## Key Principles for NSStackView Layout

1. **Distribution Property**: Must be set to `.Fill` or `.FillEqually` to make views expand
2. **Content Hugging Priority**: 
   - Fixed-height views: HIGH priority (250+) - resists expansion
   - Flexible views: LOW priority (1-10) - wants to expand
3. **Explicit Constraints**: Views without intrinsic size need explicit height constraints OR low hugging priority
4. **NSScrollView Special Case**: Has no intrinsic height, so needs either:
   - A minimum height constraint, OR
   - Very low content hugging priority

## Files Modified
- `Cargo.toml` - Added NSStackView and NSLayoutConstraint features
- `src/ui/chat_view.rs` - Full implementation with priorities and constraints
- `src/ui/history_view.rs` - Added distribution property
- `src/ui/settings_view.rs` - Added distribution property
- `src/ui/profile_editor.rs` - Added distribution property

## Verification
Build succeeds with no errors:
```bash
cargo build --bin personal_agent_menubar
```

## Testing
Run the app and click the "PA" menu bar icon to open the popover. The chat view should now display:
- Top bar with title/buttons at the top (48px)
- Chat messages area in the middle (flexible height, minimum 100px)
- Input field and send button at the bottom (50px)

Total popover size: 400x500px
