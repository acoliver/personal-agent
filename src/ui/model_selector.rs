//! Model selector view for browsing and selecting models from the registry
#![allow(unsafe_code)]
#![allow(unused_unsafe)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::match_wildcard_for_single_variants)]

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSButton, NSPopUpButton, NSScrollView, NSSearchField, NSStackView, NSStackViewDistribution,
    NSTextField, NSUserInterfaceLayoutOrientation, NSView, NSViewController,
};

use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::CALayer;

use super::model_selector_rows::has_vision;
use super::theme::Theme;

mod ui;
use ui as model_selector_ui;

use personal_agent::registry::{ModelInfo, ModelRegistry, RegistryManager};

// Thread-local storage for passing selected model to profile editor
thread_local! {
    pub static SELECTED_MODEL_PROVIDER: std::cell::Cell<Option<String>> = const { std::cell::Cell::new(None) };
    pub static SELECTED_MODEL_ID: std::cell::Cell<Option<String>> = const { std::cell::Cell::new(None) };
    pub static SELECTED_MODEL_BASE_URL: std::cell::Cell<Option<String>> = const { std::cell::Cell::new(None) };
    pub static SELECTED_MODEL_CONTEXT: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

// ============================================================================
// Helper functions for CALayer operations
// ============================================================================

fn set_layer_background_color(layer: &CALayer, r: f64, g: f64, b: f64) {
    use objc2_core_graphics::CGColor;
    let color = CGColor::new_generic_rgb(r, g, b, 1.0);
    layer.setBackgroundColor(Some(&color));
}

// ============================================================================
// Filter state
// ============================================================================

#[derive(Debug, Clone)]
struct FilterState {
    search_text: String,
    selected_provider: Option<String>,
    require_tools: bool,
    require_reasoning: bool,
    require_vision: bool,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            search_text: String::new(),
            selected_provider: None,
            require_tools: true, // Always true (not user-selectable)
            require_reasoning: false,
            require_vision: false,
        }
    }
}

// ============================================================================
// Model Selector View Controller Ivars
// ============================================================================

pub struct ModelSelectorIvars {
    // Registry data
    registry: RefCell<Option<ModelRegistry>>,

    // Filter state
    filters: RefCell<FilterState>,

    // UI components
    search_field: RefCell<Option<Retained<NSSearchField>>>,
    provider_popup: RefCell<Option<Retained<NSPopUpButton>>>,
    reasoning_checkbox: RefCell<Option<Retained<NSButton>>>,
    vision_checkbox: RefCell<Option<Retained<NSButton>>>,
    models_container: RefCell<Option<Retained<NSView>>>,
    status_label: RefCell<Option<Retained<NSTextField>>>,
    scroll_view: RefCell<Option<Retained<NSScrollView>>>,
}

// ============================================================================
// Model Selector View Controller
// ============================================================================

define_class!(
    #[unsafe(super(NSViewController))]
    #[thread_kind = MainThreadOnly]
    #[name = "ModelSelectorViewController"]
    #[ivars = ModelSelectorIvars]
    pub struct ModelSelectorViewController;

    unsafe impl NSObjectProtocol for ModelSelectorViewController {}

    impl ModelSelectorViewController {
        #[unsafe(method(loadView))]
        fn load_view(&self) {
            let mtm = MainThreadMarker::new().unwrap();

            // Create main container (400x500 for popover)
            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 500.0));
            let main_view = NSView::initWithFrame(NSView::alloc(mtm), frame);
            main_view.setWantsLayer(true);
            if let Some(layer) = main_view.layer() {
                set_layer_background_color(
                    &layer,
                    Theme::BG_DARKEST.0,
                    Theme::BG_DARKEST.1,
                    Theme::BG_DARKEST.2,
                );
            }

            // Create main vertical stack
            let main_stack = NSStackView::new(mtm);
            unsafe {
                main_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
                main_stack.setSpacing(0.0);
                main_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
                main_stack.setDistribution(NSStackViewDistribution::Fill);
            }

            // Build UI components
            let top_bar = model_selector_ui::build_top_bar(self, mtm);
            let filter_bar = model_selector_ui::build_filter_bar(self, mtm);
            let capability_toggles = model_selector_ui::build_capability_toggles(self, mtm);
            let model_list = model_selector_ui::build_model_list(self, mtm);
            let status_bar = model_selector_ui::build_status_bar(self, mtm);

            // Add to main stack
            unsafe {
                main_stack.addArrangedSubview(&top_bar);
                main_stack.addArrangedSubview(&filter_bar);
                main_stack.addArrangedSubview(&capability_toggles);
                main_stack.addArrangedSubview(&model_list);
                main_stack.addArrangedSubview(&status_bar);
            }

            // Add stack to main view
            main_view.addSubview(&main_stack);

            // Set constraints
            unsafe {
                let leading = main_stack
                    .leadingAnchor()
                    .constraintEqualToAnchor(&main_view.leadingAnchor());
                leading.setActive(true);

                let trailing = main_stack
                    .trailingAnchor()
                    .constraintEqualToAnchor(&main_view.trailingAnchor());
                trailing.setActive(true);

                let top = main_stack
                    .topAnchor()
                    .constraintEqualToAnchor(&main_view.topAnchor());
                top.setActive(true);

                let bottom = main_stack
                    .bottomAnchor()
                    .constraintEqualToAnchor(&main_view.bottomAnchor());
                bottom.setActive(true);
            }

            self.setView(&main_view);

            // Load registry and populate list
            self.load_registry();
        }

        #[unsafe(method(cancelButtonClicked:))]
        fn cancel_button_clicked(&self, _sender: Option<&NSObject>) {
            // Post notification to return to settings
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowSettingsView");
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        }

        #[unsafe(method(searchFieldChanged:))]
        fn search_field_changed(&self, _sender: Option<&NSObject>) {
            if let Some(search_field) = &*self.ivars().search_field.borrow() {
                let search_text = search_field.stringValue().to_string();
                self.ivars().filters.borrow_mut().search_text = search_text;
                self.populate_model_list();
            }
        }

        #[unsafe(method(providerPopupChanged:))]
        fn provider_popup_changed(&self, _sender: Option<&NSObject>) {
            if let Some(popup) = &*self.ivars().provider_popup.borrow() {
                let selected_index = popup.indexOfSelectedItem();

                if selected_index == 0 {
                    // "All" selected
                    self.ivars().filters.borrow_mut().selected_provider = None;
                } else if let Some(registry) = &*self.ivars().registry.borrow() {
                    let provider_ids = registry.get_provider_ids();
                    if (selected_index as usize) <= provider_ids.len() {
                        let provider_id = provider_ids[(selected_index - 1) as usize].clone();
                        self.ivars().filters.borrow_mut().selected_provider = Some(provider_id);
                    }
                }

                self.populate_model_list();
            }
        }

        #[unsafe(method(reasoningCheckboxToggled:))]
        fn reasoning_checkbox_toggled(&self, sender: Option<&NSObject>) {
            if let Some(checkbox) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                self.ivars().filters.borrow_mut().require_reasoning = checkbox.state() == 1;
                self.populate_model_list();
            }
        }

        #[unsafe(method(visionCheckboxToggled:))]
        fn vision_checkbox_toggled(&self, sender: Option<&NSObject>) {
            if let Some(checkbox) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                self.ivars().filters.borrow_mut().require_vision = checkbox.state() == 1;
                self.populate_model_list();
            }
        }

        #[unsafe(method(modelSelected:))]
        fn model_selected(&self, sender: Option<&NSObject>) {
            // Get the button's tag which encodes provider and model indices
            // IMPORTANT: These indices are into the FILTERED grouped_models list,
            // not the full registry.get_provider_ids() list
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                let tag = button.tag();
                let provider_index = (tag / 1000) as usize;
                let model_index = (tag % 1000) as usize;

                if let Some(registry) = &*self.ivars().registry.borrow() {
                    // Re-compute the filtered models using current filter state
                    let filters = self.ivars().filters.borrow().clone();
                    let grouped_models = Self::get_filtered_models(registry, &filters);

                    if provider_index < grouped_models.len() {
                        let (provider_id, models) = &grouped_models[provider_index];
                        if model_index < models.len() {
                            let model_id = &models[model_index].id;

                            println!("Selected model: {provider_id}:{model_id}");

                            // Post notification with selection
                            self.post_model_selected_notification(provider_id, model_id);
                        }
                    }
                }
            }
        }
    }
);

impl ModelSelectorViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let ivars = ModelSelectorIvars {
            registry: RefCell::new(None),
            filters: RefCell::new(FilterState::default()),
            search_field: RefCell::new(None),
            provider_popup: RefCell::new(None),
            reasoning_checkbox: RefCell::new(None),
            vision_checkbox: RefCell::new(None),
            models_container: RefCell::new(None),
            status_label: RefCell::new(None),
            scroll_view: RefCell::new(None),
        };

        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }

    fn load_registry(&self) {
        let manager = match RegistryManager::new() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Failed to create registry manager: {e}");
                return;
            }
        };

        let runtime = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to create runtime: {e}");
                return;
            }
        };

        let registry = runtime.block_on(async { manager.get_registry().await });

        match registry {
            Ok(reg) => {
                *self.ivars().registry.borrow_mut() = Some(reg);
                self.populate_provider_popup();
                self.populate_model_list();
            }
            Err(e) => {
                eprintln!("Failed to load registry: {e}");
                self.show_error("Failed to load registry");
            }
        }
    }

    fn populate_provider_popup(&self) {
        if let Some(popup) = &*self.ivars().provider_popup.borrow() {
            if let Some(registry) = &*self.ivars().registry.borrow() {
                let provider_ids = registry.get_provider_ids();

                for provider_id in provider_ids {
                    if let Some(provider) = registry.get_provider(&provider_id) {
                        popup.addItemWithTitle(&NSString::from_str(&provider.name));
                    }
                }
            }
        }
    }

    fn populate_model_list(&self) {
        let mtm = MainThreadMarker::new().unwrap();

        if let Some(container) = &*self.ivars().models_container.borrow() {
            // Clear existing views
            let subviews = container.subviews();
            for view in &subviews {
                if let Some(stack) = container.downcast_ref::<NSStackView>() {
                    unsafe {
                        stack.removeArrangedSubview(&view);
                    }
                }
                view.removeFromSuperview();
            }

            if let Some(registry) = &*self.ivars().registry.borrow() {
                let filters = self.ivars().filters.borrow();

                // Get filtered models grouped by provider
                let grouped_models = Self::get_filtered_models(registry, &filters);

                if grouped_models.is_empty() {
                    self.show_empty_state();
                } else {
                    // Count total models and providers
                    let total_models: usize =
                        grouped_models.iter().map(|(_, models)| models.len()).sum();
                    let total_providers = grouped_models.len();

                    // Update status label
                    if let Some(status_label) = &*self.ivars().status_label.borrow() {
                        let status_text =
                            format!("{total_models} models from {total_providers} providers");
                        status_label.setStringValue(&NSString::from_str(&status_text));
                    }

                    // Add provider sections
                    if let Some(stack) = container.downcast_ref::<NSStackView>() {
                        for (provider_index, (provider_id, models)) in
                            grouped_models.iter().enumerate()
                        {
                            // Add provider header
                            if let Some(provider) = registry.get_provider(provider_id) {
                                let header = self.create_provider_header(&provider.name, mtm);
                                unsafe {
                                    stack.addArrangedSubview(&header);
                                }
                            }

                            // Add model rows
                            for (model_index, model) in models.iter().enumerate() {
                                let row =
                                    self.create_model_row(model, provider_index, model_index, mtm);
                                unsafe {
                                    stack.addArrangedSubview(&row);
                                }
                            }

                            // Add spacing between provider sections
                            let spacer = NSView::initWithFrame(
                                NSView::alloc(mtm),
                                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 8.0)),
                            );
                            unsafe {
                                spacer.setTranslatesAutoresizingMaskIntoConstraints(false);
                                let height = spacer.heightAnchor().constraintEqualToConstant(8.0);
                                height.setActive(true);
                                stack.addArrangedSubview(&spacer);
                            }
                        }
                    }
                }
            }
        }

        // Scroll to top - force layout first
        if let Some(scroll_view) = &*self.ivars().scroll_view.borrow() {
            scroll_view.layoutSubtreeIfNeeded();
            let clip_view = scroll_view.contentView();
            clip_view.scrollToPoint(NSPoint::new(0.0, 0.0));
            scroll_view.reflectScrolledClipView(&clip_view);
        }
    }

    fn get_filtered_models<'a>(
        registry: &'a ModelRegistry,
        filters: &FilterState,
    ) -> Vec<(String, Vec<&'a ModelInfo>)> {
        let mut result = Vec::new();

        let provider_ids = if let Some(ref provider_id) = filters.selected_provider {
            vec![provider_id.clone()]
        } else {
            registry.get_provider_ids()
        };

        for provider_id in provider_ids {
            if let Some(models) = registry.get_models_for_provider(&provider_id) {
                let filtered: Vec<&ModelInfo> = models
                    .into_iter()
                    .filter(|model| Self::model_matches_filters(model, filters))
                    .collect();

                if !filtered.is_empty() {
                    result.push((provider_id, filtered));
                }
            }
        }

        result
    }

    fn model_matches_filters(model: &ModelInfo, filters: &FilterState) -> bool {
        // Tool call filter (required by default)
        if filters.require_tools && !model.tool_call {
            return false;
        }

        // Reasoning filter
        if filters.require_reasoning && !model.reasoning {
            return false;
        }

        // Vision filter
        if filters.require_vision && !has_vision(model) {
            return false;
        }

        // Search filter
        if !filters.search_text.is_empty() {
            let search_lower = filters.search_text.to_lowercase();
            let id_match = model.id.to_lowercase().contains(&search_lower);
            let name_match = model.name.to_lowercase().contains(&search_lower);
            if !id_match && !name_match {
                return false;
            }
        }

        true
    }

    fn create_provider_header(
        &self,
        provider_name: &str,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        model_selector_ui::create_provider_header(self, provider_name, mtm)
    }

    pub(super) fn create_model_row(
        &self,
        model: &ModelInfo,
        provider_index: usize,
        model_index: usize,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        model_selector_ui::create_model_row(self, model, provider_index, model_index, mtm)
    }

    fn show_empty_state(&self) {
        model_selector_ui::show_empty_state(self);
    }

    fn show_error(&self, message: &str) {
        model_selector_ui::show_error(self, message);
    }

    fn post_model_selected_notification(&self, provider_id: &str, model_id: &str) {
        model_selector_ui::post_model_selected_notification(self, provider_id, model_id);
    }
}
