//! UI row builders for the model selector.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSFont, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::ui::theme::Theme;
use personal_agent::registry::ModelInfo;

use super::set_layer_background_color;

pub struct ModelSelectorRowHelper;

impl ModelSelectorRowHelper {
    pub fn create_provider_header(provider_name: &str, mtm: MainThreadMarker) -> Retained<NSView> {
        let header = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(376.0, 24.0)),
        );
        header.setWantsLayer(true);
        if let Some(layer) = header.layer() {
            set_layer_background_color(&layer, 0.12, 0.12, 0.12);
        }

        unsafe {
            header.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width = header.widthAnchor().constraintEqualToConstant(376.0);
            let height = header.heightAnchor().constraintEqualToConstant(24.0);
            width.setActive(true);
            height.setActive(true);
        }

        let label = NSTextField::labelWithString(&NSString::from_str(provider_name), mtm);
        label.setTextColor(Some(&Theme::text_primary()));
        label.setFont(Some(&NSFont::boldSystemFontOfSize(12.0)));
        unsafe {
            label.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        header.addSubview(&label);

        unsafe {
            let leading = label
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&header.leadingAnchor(), 8.0);
            let center_y = label
                .centerYAnchor()
                .constraintEqualToAnchor(&header.centerYAnchor());
            leading.setActive(true);
            center_y.setActive(true);
        }

        header
    }
}

pub(super) fn has_vision(model: &ModelInfo) -> bool {
    model
        .modalities
        .as_ref()
        .is_some_and(|m| m.input.iter().any(|input| input == "image"))
}
