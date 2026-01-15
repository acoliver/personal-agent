//! Model selector view for browsing and selecting models from the registry

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly, DefinedClass};
use objc2_foundation::{
    NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSDictionary, NSNumber,
};
use objc2_app_kit::{
    NSView, NSViewController, NSTextField, NSButton, NSScrollView, NSFont, NSBezelStyle,
    NSStackView, NSUserInterfaceLayoutOrientation, NSStackViewDistribution,
    NSLayoutConstraintOrientation, NSButtonType, NSPopUpButton, NSSearchField,
};
use objc2_quartz_core::CALayer;

use super::theme::Theme;
use personal_agent::registry::{ModelRegistry, RegistryManager, ModelInfo};

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

fn set_layer_corner_radius(layer: &CALayer, radius: f64) {
    layer.setCornerRadius(radius);
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
                set_layer_background_color(&layer, Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2);
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
            let top_bar = self.build_top_bar(mtm);
            let filter_bar = self.build_filter_bar(mtm);
            let capability_toggles = self.build_capability_toggles(mtm);
            let model_list = self.build_model_list(mtm);
            let status_bar = self.build_status_bar(mtm);
            
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
                let leading = main_stack.leadingAnchor().constraintEqualToAnchor(&main_view.leadingAnchor());
                leading.setActive(true);
                
                let trailing = main_stack.trailingAnchor().constraintEqualToAnchor(&main_view.trailingAnchor());
                trailing.setActive(true);
                
                let top = main_stack.topAnchor().constraintEqualToAnchor(&main_view.topAnchor());
                top.setActive(true);
                
                let bottom = main_stack.bottomAnchor().constraintEqualToAnchor(&main_view.bottomAnchor());
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
                    let grouped_models = self.get_filtered_models(registry, &filters);
                    
                    if provider_index < grouped_models.len() {
                        let (provider_id, models) = &grouped_models[provider_index];
                        if model_index < models.len() {
                            let model_id = &models[model_index].id;
                            
                            println!("Selected model: {}:{}", provider_id, model_id);
                            
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

    fn build_top_bar(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let top_bar = NSStackView::new(mtm);
        unsafe {
            top_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            top_bar.setSpacing(8.0);
            top_bar.setTranslatesAutoresizingMaskIntoConstraints(false);
            top_bar.setDistribution(NSStackViewDistribution::Fill);
            top_bar.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 8.0,
                left: 12.0,
                bottom: 8.0,
                right: 12.0,
            });
        }
        
        top_bar.setWantsLayer(true);
        if let Some(layer) = top_bar.layer() {
            set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
        }
        
        unsafe {
            top_bar.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            let height_constraint = top_bar.heightAnchor().constraintEqualToConstant(44.0);
            height_constraint.setActive(true);
        }

        // Cancel button
        let cancel_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Cancel"),
                Some(self),
                Some(sel!(cancelButtonClicked:)),
                mtm,
            )
        };
        cancel_btn.setBezelStyle(NSBezelStyle::Rounded);
        unsafe {
            cancel_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            cancel_btn.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
            let width_constraint = cancel_btn.widthAnchor().constraintEqualToConstant(70.0);
            width_constraint.setActive(true);
        }
        unsafe {
            top_bar.addArrangedSubview(&cancel_btn);
        }

        // Spacer
        let spacer1 = NSView::new(mtm);
        unsafe {
            spacer1.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
            top_bar.addArrangedSubview(&spacer1);
        }

        // Title
        let title = NSTextField::labelWithString(&NSString::from_str("Select Model"), mtm);
        title.setTextColor(Some(&Theme::text_primary()));
        title.setFont(Some(&NSFont::boldSystemFontOfSize(14.0)));
        title.setAlignment(objc2_app_kit::NSTextAlignment::Center);
        unsafe {
            title.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
            top_bar.addArrangedSubview(&title);
        }

        // Spacer
        let spacer2 = NSView::new(mtm);
        unsafe {
            spacer2.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
            top_bar.addArrangedSubview(&spacer2);
        }

        Retained::from(&*top_bar as &NSView)
    }

    fn build_filter_bar(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let filter_bar = NSStackView::new(mtm);
        unsafe {
            filter_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            filter_bar.setSpacing(8.0);
            filter_bar.setTranslatesAutoresizingMaskIntoConstraints(false);
            filter_bar.setDistribution(NSStackViewDistribution::Fill);
            filter_bar.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 8.0,
                left: 12.0,
                bottom: 8.0,
                right: 12.0,
            });
        }
        
        filter_bar.setWantsLayer(true);
        if let Some(layer) = filter_bar.layer() {
            set_layer_background_color(&layer, Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2);
        }
        
        unsafe {
            filter_bar.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            let height_constraint = filter_bar.heightAnchor().constraintEqualToConstant(36.0);
            height_constraint.setActive(true);
        }

        // Search field (flexible width)
        let search_field = NSSearchField::initWithFrame(
            NSSearchField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(240.0, 24.0)),
        );
        search_field.setPlaceholderString(Some(&NSString::from_str("Search models...")));
        search_field.setBackgroundColor(Some(&Theme::bg_darker()));
        search_field.setTextColor(Some(&Theme::text_primary()));
        unsafe {
            search_field.setTarget(Some(self));
            search_field.setAction(Some(sel!(searchFieldChanged:)));
            search_field.setTranslatesAutoresizingMaskIntoConstraints(false);
            search_field.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
        }
        unsafe {
            filter_bar.addArrangedSubview(&search_field);
        }
        *self.ivars().search_field.borrow_mut() = Some(search_field);

        // Provider label
        let provider_label = NSTextField::labelWithString(&NSString::from_str("Provider:"), mtm);
        provider_label.setTextColor(Some(&Theme::text_secondary_color()));
        provider_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            provider_label.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
            filter_bar.addArrangedSubview(&provider_label);
        }

        // Provider popup
        let provider_popup = NSPopUpButton::new(mtm);
        provider_popup.addItemWithTitle(&NSString::from_str("All"));
        unsafe {
            provider_popup.setTarget(Some(self));
            provider_popup.setAction(Some(sel!(providerPopupChanged:)));
            provider_popup.setTranslatesAutoresizingMaskIntoConstraints(false);
            provider_popup.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
            let width_constraint = provider_popup.widthAnchor().constraintEqualToConstant(100.0);
            width_constraint.setActive(true);
        }
        unsafe {
            filter_bar.addArrangedSubview(&provider_popup);
        }
        *self.ivars().provider_popup.borrow_mut() = Some(provider_popup);

        Retained::from(&*filter_bar as &NSView)
    }

    fn build_capability_toggles(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let toggles_bar = NSStackView::new(mtm);
        unsafe {
            toggles_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            toggles_bar.setSpacing(12.0);
            toggles_bar.setTranslatesAutoresizingMaskIntoConstraints(false);
            toggles_bar.setDistribution(NSStackViewDistribution::Fill);
            toggles_bar.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 6.0,
                left: 12.0,
                bottom: 6.0,
                right: 12.0,
            });
        }
        
        toggles_bar.setWantsLayer(true);
        if let Some(layer) = toggles_bar.layer() {
            set_layer_background_color(&layer, Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2);
        }
        
        unsafe {
            toggles_bar.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            let height_constraint = toggles_bar.heightAnchor().constraintEqualToConstant(28.0);
            height_constraint.setActive(true);
        }

        // Reasoning checkbox
        let reasoning_checkbox = NSButton::initWithFrame(
            NSButton::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(100.0, 20.0)),
        );
        reasoning_checkbox.setButtonType(NSButtonType::Switch);
        reasoning_checkbox.setTitle(&NSString::from_str("Reasoning"));
        reasoning_checkbox.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            reasoning_checkbox.setTarget(Some(self));
            reasoning_checkbox.setAction(Some(sel!(reasoningCheckboxToggled:)));
            toggles_bar.addArrangedSubview(&reasoning_checkbox);
        }
        *self.ivars().reasoning_checkbox.borrow_mut() = Some(reasoning_checkbox);

        // Vision checkbox
        let vision_checkbox = NSButton::initWithFrame(
            NSButton::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(80.0, 20.0)),
        );
        vision_checkbox.setButtonType(NSButtonType::Switch);
        vision_checkbox.setTitle(&NSString::from_str("Vision"));
        vision_checkbox.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            vision_checkbox.setTarget(Some(self));
            vision_checkbox.setAction(Some(sel!(visionCheckboxToggled:)));
            toggles_bar.addArrangedSubview(&vision_checkbox);
        }
        *self.ivars().vision_checkbox.borrow_mut() = Some(vision_checkbox);

        // Spacer to push checkboxes left
        let spacer = NSView::new(mtm);
        unsafe {
            spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
            toggles_bar.addArrangedSubview(&spacer);
        }

        Retained::from(&*toggles_bar as &NSView)
    }

    fn build_model_list(&self, mtm: MainThreadMarker) -> Retained<NSScrollView> {
        let scroll_view = NSScrollView::new(mtm);
        scroll_view.setHasVerticalScroller(true);
        scroll_view.setDrawsBackground(false);
        unsafe {
            scroll_view.setAutohidesScrollers(true);
            scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        unsafe {
            scroll_view.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
            scroll_view.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Vertical);
            let min_height = scroll_view.heightAnchor().constraintGreaterThanOrEqualToConstant(100.0);
            min_height.setActive(true);
        }

        // Models stack
        // Use FlippedStackView so models start at TOP (not bottom)
        let models_stack = super::FlippedStackView::new(mtm);
        unsafe {
            models_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            models_stack.setSpacing(0.0);
            models_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Width);
            // Fill distribution works correctly with flipped coordinates
            models_stack.setDistribution(NSStackViewDistribution::Fill);
            models_stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 0.0,
                left: 0.0,
                bottom: 0.0,
                right: 0.0,
            });
        }
        
        models_stack.setWantsLayer(true);
        if let Some(layer) = models_stack.layer() {
            set_layer_background_color(&layer, Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2);
        }
        
        models_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        scroll_view.setDocumentView(Some(&models_stack));
        
        // Constrain width
        let content_view = scroll_view.contentView();
        let width_constraint = models_stack.widthAnchor().constraintEqualToAnchor_constant(&content_view.widthAnchor(), -24.0);
        width_constraint.setActive(true);

        *self.ivars().scroll_view.borrow_mut() = Some(scroll_view.clone());
        *self.ivars().models_container.borrow_mut() = Some(Retained::from(&*models_stack as &NSView));

        scroll_view
    }

    fn build_status_bar(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let status_bar = NSStackView::new(mtm);
        unsafe {
            status_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            status_bar.setTranslatesAutoresizingMaskIntoConstraints(false);
            status_bar.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 6.0,
                left: 12.0,
                bottom: 6.0,
                right: 12.0,
            });
        }
        
        status_bar.setWantsLayer(true);
        if let Some(layer) = status_bar.layer() {
            set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
        }
        
        unsafe {
            status_bar.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            let height_constraint = status_bar.heightAnchor().constraintEqualToConstant(24.0);
            height_constraint.setActive(true);
        }

        // Status label
        let status_label = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        status_label.setTextColor(Some(&Theme::text_secondary_color()));
        status_label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        unsafe {
            status_bar.addArrangedSubview(&status_label);
        }
        *self.ivars().status_label.borrow_mut() = Some(status_label);

        Retained::from(&*status_bar as &NSView)
    }

    fn load_registry(&self) {
        let manager = match RegistryManager::new() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Failed to create registry manager: {}", e);
                return;
            }
        };
        
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to create runtime: {}", e);
                return;
            }
        };
        
        let registry = runtime.block_on(async {
            manager.get_registry().await
        });
        
        match registry {
            Ok(reg) => {
                *self.ivars().registry.borrow_mut() = Some(reg);
                self.populate_provider_popup();
                self.populate_model_list();
            }
            Err(e) => {
                eprintln!("Failed to load registry: {}", e);
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
            for view in subviews.iter() {
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
                let grouped_models = self.get_filtered_models(registry, &filters);
                
                if grouped_models.is_empty() {
                    self.show_empty_state();
                } else {
                    // Count total models and providers
                    let total_models: usize = grouped_models.iter().map(|(_, models)| models.len()).sum();
                    let total_providers = grouped_models.len();
                    
                    // Update status label
                    if let Some(status_label) = &*self.ivars().status_label.borrow() {
                        let status_text = format!("{} models from {} providers", total_models, total_providers);
                        status_label.setStringValue(&NSString::from_str(&status_text));
                    }
                    
                    // Add provider sections
                    if let Some(stack) = container.downcast_ref::<NSStackView>() {
                        for (provider_index, (provider_id, models)) in grouped_models.iter().enumerate() {
                            // Add provider header
                            if let Some(provider) = registry.get_provider(provider_id) {
                                let header = self.create_provider_header(&provider.name, mtm);
                                unsafe {
                                    stack.addArrangedSubview(&header);
                                }
                            }
                            
                            // Add model rows
                            for (model_index, model) in models.iter().enumerate() {
                                let row = self.create_model_row(model, provider_index, model_index, mtm);
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
        &self,
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
                    .filter(|model| self.model_matches_filters(model, filters))
                    .collect();
                
                if !filtered.is_empty() {
                    result.push((provider_id, filtered));
                }
            }
        }
        
        result
    }

    fn model_matches_filters(&self, model: &ModelInfo, filters: &FilterState) -> bool {
        // Tool call filter (required by default)
        if filters.require_tools && !model.tool_call {
            return false;
        }
        
        // Reasoning filter
        if filters.require_reasoning && !model.reasoning {
            return false;
        }
        
        // Vision filter
        if filters.require_vision {
            let has_vision = model.modalities
                .as_ref()
                .map(|m| m.input.iter().any(|input| input == "image"))
                .unwrap_or(false);
            if !has_vision {
                return false;
            }
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

    fn create_provider_header(&self, provider_name: &str, mtm: MainThreadMarker) -> Retained<NSView> {
        let header = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(376.0, 24.0)),
        );
        header.setWantsLayer(true);
        if let Some(layer) = header.layer() {
            set_layer_background_color(&layer, 0.12, 0.12, 0.12); // Slightly lighter than BG_DARKEST
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
        
        // Position label with padding
        unsafe {
            let leading = label.leadingAnchor().constraintEqualToAnchor_constant(&header.leadingAnchor(), 8.0);
            let center_y = label.centerYAnchor().constraintEqualToAnchor(&header.centerYAnchor());
            leading.setActive(true);
            center_y.setActive(true);
        }
        
        header
    }

    fn create_model_row(
        &self,
        model: &ModelInfo,
        provider_index: usize,
        model_index: usize,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        // Create button that acts as the row
        let button = NSButton::initWithFrame(
            NSButton::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(376.0, 28.0)),
        );
        button.setButtonType(NSButtonType::MomentaryPushIn);
        button.setBezelStyle(NSBezelStyle::Rounded);
        button.setBordered(true);
        
        // Build row title: model_id | context | caps | cost
        let mut title_parts = vec![model.id.clone()];
        
        // Context
        if let Some(limit) = &model.limit {
            title_parts.push(format_context_window(limit.context));
        }
        
        // Capabilities
        let mut caps = String::new();
        if model.reasoning {
            caps.push('R');
        }
        if has_vision(model) {
            if !caps.is_empty() {
                caps.push(' ');
            }
            caps.push('V');
        }
        if !caps.is_empty() {
            title_parts.push(caps);
        }
        
        // Cost
        if let Some(cost) = &model.cost {
            title_parts.push(format_cost(cost.input, cost.output));
        }
        
        let title = title_parts.join(" | ");
        button.setTitle(&NSString::from_str(&title));
        button.setAlignment(objc2_app_kit::NSTextAlignment::Left);
        button.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        
        // Encode provider and model indices in tag
        let tag = (provider_index * 1000 + model_index) as isize;
        button.setTag(tag);
        
        unsafe {
            button.setTarget(Some(self));
            button.setAction(Some(sel!(modelSelected:)));
            button.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width = button.widthAnchor().constraintEqualToConstant(376.0);
            let height = button.heightAnchor().constraintEqualToConstant(28.0);
            width.setActive(true);
            height.setActive(true);
        }
        
        Retained::from(&*button as &NSView)
    }

    fn show_empty_state(&self) {
        let mtm = MainThreadMarker::new().unwrap();
        
        if let Some(container) = &*self.ivars().models_container.borrow() {
            if let Some(stack) = container.downcast_ref::<NSStackView>() {
                let message = NSTextField::labelWithString(
                    &NSString::from_str("No models match your filters.\n\nTry adjusting the capability\nfilters or search term."),
                    mtm,
                );
                message.setTextColor(Some(&Theme::text_secondary_color()));
                message.setFont(Some(&NSFont::systemFontOfSize(13.0)));
                message.setAlignment(objc2_app_kit::NSTextAlignment::Center);
                unsafe {
                    stack.addArrangedSubview(&message);
                }
            }
        }
        
        // Update status label
        if let Some(status_label) = &*self.ivars().status_label.borrow() {
            status_label.setStringValue(&NSString::from_str("0 models from 0 providers"));
        }
    }

    fn show_error(&self, message: &str) {
        let mtm = MainThreadMarker::new().unwrap();
        
        if let Some(container) = &*self.ivars().models_container.borrow() {
            if let Some(stack) = container.downcast_ref::<NSStackView>() {
                let label = NSTextField::labelWithString(&NSString::from_str(message), mtm);
                label.setTextColor(Some(&Theme::text_secondary_color()));
                label.setFont(Some(&NSFont::systemFontOfSize(13.0)));
                unsafe {
                    stack.addArrangedSubview(&label);
                }
            }
        }
    }

    fn post_model_selected_notification(&self, provider_id: &str, model_id: &str) {
        use objc2_foundation::NSNotificationCenter;
        
        let center = NSNotificationCenter::defaultCenter();
        let name = NSString::from_str("PersonalAgentModelSelected");
        
        // Store selected model in thread-local for profile editor to read
        SELECTED_MODEL_PROVIDER.with(|cell| {
            cell.set(Some(provider_id.to_string()));
        });
        SELECTED_MODEL_ID.with(|cell| {
            cell.set(Some(model_id.to_string()));
        });
        
        // Also store base_url and context from the registry
        if let Some(registry) = &*self.ivars().registry.borrow() {
            // Get provider's API URL
            if let Some(provider) = registry.get_provider(provider_id) {
                SELECTED_MODEL_BASE_URL.with(|cell| {
                    cell.set(provider.api.clone());
                });
            }
            
            // Get model's context limit
            if let Some(models) = registry.get_models_for_provider(provider_id) {
                if let Some(model) = models.iter().find(|m| m.id == model_id) {
                    if let Some(limit) = &model.limit {
                        SELECTED_MODEL_CONTEXT.with(|cell| {
                            cell.set(Some(limit.context));
                        });
                    }
                }
            }
        }
        
        println!("Model selected notification: {}:{}", provider_id, model_id);
        
        unsafe {
            center.postNotificationName_object(&name, None);
        }
    }
}

// ============================================================================
// Helper functions
// ============================================================================

fn format_context_window(context: u64) -> String {
    if context >= 1_000_000 {
        format!("{}M", context / 1_000_000)
    } else if context >= 1_000 {
        format!("{}K", context / 1_000)
    } else {
        context.to_string()
    }
}

fn format_cost(input: f64, output: f64) -> String {
    if input == 0.0 && output == 0.0 {
        "free".to_string()
    } else {
        // Convert to cost per million tokens (registry uses per-token cost)
        let input_per_m = input * 1_000_000.0;
        let output_per_m = output * 1_000_000.0;
        
        // Round to 1 decimal, drop trailing zeros
        let input_str = format!("{:.1}", input_per_m).trim_end_matches(".0").to_string();
        let output_str = format!("{:.1}", output_per_m).trim_end_matches(".0").to_string();
        
        format!("{}/{}", input_str, output_str)
    }
}

fn has_vision(model: &ModelInfo) -> bool {
    model.modalities
        .as_ref()
        .map(|m| m.input.iter().any(|input| input == "image"))
        .unwrap_or(false)
}
