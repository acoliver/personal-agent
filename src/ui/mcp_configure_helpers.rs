use std::collections::HashMap;

use objc2::rc::Retained;
use objc2::{sel, DefinedClass, MainThreadMarker};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSFont, NSPopUpButton, NSStackView, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSEdgeInsets, NSString};
use uuid::Uuid;

use crate::ui::Theme;
use personal_agent::config::Config;
use personal_agent::mcp::{
    EnvVarConfig, McpAuthType, McpConfig, McpPackage, McpPackageArg, McpPackageType, McpSource,
    McpTransport, SecretsManager,
};

use super::mcp_configure_view::log_to_file;
use crate::ui::mcp_add_helpers::ParsedMcp;

pub struct ManualConfigInput {
    pub auth_type: McpAuthType,
    pub keyfile_path: Option<std::path::PathBuf>,
}

pub fn save_oauth_token(config_id: Uuid, token: &str) -> Result<(), String> {
    let config_path =
        Config::default_path().map_err(|e| format!("Failed to get config path: {e}"))?;

    let mut config =
        Config::load(&config_path).map_err(|e| format!("Failed to load config: {e}"))?;

    if let Some(mcp) = config.mcps.iter_mut().find(|m| m.id == config_id) {
        mcp.oauth_token = Some(token.to_string());
    } else {
        return Err("MCP config not found".to_string());
    }

    config
        .save(&config_path)
        .map_err(|e| format!("Failed to save config: {e}"))?;

    Ok(())
}

pub fn build_env_values(
    inputs: &[(String, Retained<NSTextField>)],
    secrets_manager: &SecretsManager,
    config_id: Uuid,
) -> Result<HashMap<String, String>, String> {
    let mut env_values = HashMap::new();

    for (var_name, field) in inputs {
        let value = field.stringValue().to_string().trim().to_string();
        if value.is_empty() {
            continue;
        }

        env_values.insert(var_name.clone(), value.clone());

        let var_name_lower = var_name.to_lowercase();
        let is_secret = var_name_lower.contains("key")
            || var_name_lower.contains("secret")
            || var_name_lower.contains("token")
            || var_name_lower.contains("password")
            || var_name_lower.contains("pat");

        if is_secret {
            log_to_file(&format!("Storing secret for {var_name}"));
            secrets_manager
                .store_api_key_named(config_id, var_name, &value)
                .map_err(|e| format!("Failed to store secret {var_name}: {e}"))?;
        }
    }

    Ok(env_values)
}

pub fn build_package_arg_values(
    inputs: &[(String, Retained<NSTextField>)],
) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for (arg_name, field) in inputs {
        let value = field.stringValue().to_string().trim().to_string();
        if !value.is_empty() {
            values.insert(arg_name.clone(), value);
        }
    }
    values
}

pub fn validate_required_env_vars(
    env_vars: &[EnvVarConfig],
    values: &HashMap<String, String>,
) -> Option<String> {
    for env_var in env_vars {
        if env_var.required && !values.contains_key(&env_var.name) {
            return Some(env_var.name.clone());
        }
    }
    None
}

pub fn validate_required_package_args(
    package_args: &[McpPackageArg],
    values: &HashMap<String, String>,
) -> Option<String> {
    for arg in package_args {
        if arg.required && !values.contains_key(&arg.name) {
            return Some(arg.name.clone());
        }
    }
    None
}

pub fn merge_config_values(
    config: &mut McpConfig,
    env_values: &HashMap<String, String>,
    package_args: &HashMap<String, String>,
) {
    let existing_config = config.config.as_object();
    let mut config_json = existing_config.cloned().unwrap_or_default();

    if !env_values.is_empty() {
        config_json.insert(
            "env_vars".to_string(),
            serde_json::to_value(env_values).unwrap_or_default(),
        );
    }

    if !package_args.is_empty() {
        config_json.insert(
            "package_args".to_string(),
            serde_json::to_value(package_args).unwrap_or_default(),
        );
    }

    config.config = serde_json::Value::Object(config_json);
}

pub fn parse_manual_auth_inputs(
    auth_type_popup: Option<&Retained<NSPopUpButton>>,
    _api_key_input: Option<&Retained<NSTextField>>,
    keyfile_input: Option<&Retained<NSTextField>>,
) -> ManualConfigInput {
    let auth_type = auth_type_popup.as_ref().map_or(McpAuthType::None, |popup| {
        match popup.indexOfSelectedItem() {
            0 => McpAuthType::ApiKey,
            1 => McpAuthType::Keyfile,
            _ => McpAuthType::None,
        }
    });

    let keyfile_path = keyfile_input.and_then(|field| {
        let path_str = field.stringValue().to_string().trim().to_string();
        if path_str.is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(path_str))
        }
    });

    ManualConfigInput {
        auth_type,
        keyfile_path,
    }
}

pub fn build_manual_mcp_config(
    parsed: &ParsedMcp,
    name: String,
    auth: ManualConfigInput,
) -> McpConfig {
    let (package, source) = match parsed {
        ParsedMcp::Npm {
            identifier,
            runtime_hint,
        } => {
            let package = McpPackage {
                package_type: McpPackageType::Npm,
                identifier: identifier.clone(),
                runtime_hint: Some(runtime_hint.clone()),
            };
            let source = McpSource::Manual {
                url: format!("npx {identifier}"),
            };
            (package, source)
        }
        ParsedMcp::Docker { image } => {
            let package = McpPackage {
                package_type: McpPackageType::Docker,
                identifier: image.clone(),
                runtime_hint: None,
            };
            let source = McpSource::Manual {
                url: format!("docker run {image}"),
            };
            (package, source)
        }
        ParsedMcp::Http { url } => {
            let package = McpPackage {
                package_type: McpPackageType::Http,
                identifier: url.clone(),
                runtime_hint: None,
            };
            let source = McpSource::Manual { url: url.clone() };
            (package, source)
        }
    };

    let transport = match package.package_type {
        McpPackageType::Http => McpTransport::Http,
        _ => McpTransport::Stdio,
    };

    McpConfig {
        id: Uuid::new_v4(),
        name,
        enabled: true,
        source,
        package,
        transport,
        auth_type: auth.auth_type,
        env_vars: vec![],
        package_args: vec![],
        keyfile_path: auth.keyfile_path,
        config: serde_json::json!({}),
        oauth_token: None,
    }
}

pub fn build_form_stack(mtm: MainThreadMarker) -> Retained<super::FlippedStackView> {
    let form_stack = super::FlippedStackView::new(mtm);
    form_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    form_stack.setSpacing(16.0);
    form_stack.setEdgeInsets(NSEdgeInsets {
        top: 16.0,
        left: 16.0,
        bottom: 16.0,
        right: 16.0,
    });
    form_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
    form_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
    form_stack
}

pub fn form_default_name(selected: Option<&McpConfig>, parsed: Option<&ParsedMcp>) -> String {
    if let Some(config) = selected {
        return config.name.clone();
    }

    if let Some(parsed) = parsed {
        return match parsed {
            ParsedMcp::Npm { identifier, .. } => identifier
                .split('/')
                .next_back()
                .unwrap_or(identifier)
                .to_string(),
            ParsedMcp::Docker { image } => image
                .split(':')
                .next()
                .unwrap_or(image)
                .split('/')
                .next_back()
                .unwrap_or(image)
                .to_string(),
            ParsedMcp::Http { url } => url.split('/').next_back().unwrap_or("mcp").to_string(),
        };
    }

    "My MCP".to_string()
}

pub fn build_oauth_section(
    controller: &super::mcp_configure_view::McpConfigureViewController,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let container = NSStackView::new(mtm);
    container.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    container.setSpacing(12.0);
    container.setTranslatesAutoresizingMaskIntoConstraints(false);

    let label = NSTextField::labelWithString(&NSString::from_str("Authentication"), mtm);
    label.setTextColor(Some(&Theme::text_primary()));
    label.setFont(Some(&NSFont::boldSystemFontOfSize(12.0)));
    container.addArrangedSubview(&label);

    let is_connected = controller
        .ivars()
        .selected_config
        .borrow()
        .as_ref()
        .and_then(|c| c.oauth_token.as_ref())
        .is_some();

    let status_text = if is_connected {
        "Connected [OK]"
    } else {
        "Not connected"
    };
    let status = NSTextField::labelWithString(&NSString::from_str(status_text), mtm);
    status.setTextColor(Some(&Theme::text_primary()));
    status.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    container.addArrangedSubview(&status);
    *controller.ivars().oauth_status_label.borrow_mut() = Some(status);

    let btn = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Connect with Smithery"),
            Some(controller),
            Some(sel!(connectSmitheryClicked:)),
            mtm,
        )
    };
    btn.setBezelStyle(NSBezelStyle::Automatic);
    btn.setEnabled(!is_connected);
    let btn_width = btn
        .widthAnchor()
        .constraintGreaterThanOrEqualToConstant(350.0);
    btn_width.setActive(true);
    container.addArrangedSubview(&btn);
    *controller.ivars().oauth_button.borrow_mut() = Some(btn);

    let info = NSTextField::labelWithString(
        &NSString::from_str("Click to authorize this application with Smithery"),
        mtm,
    );
    info.setTextColor(Some(&Theme::text_secondary_color()));
    info.setFont(Some(&NSFont::systemFontOfSize(10.0)));
    container.addArrangedSubview(&info);

    Retained::from(&*container as &NSView)
}

pub fn build_auth_section(
    controller: &super::mcp_configure_view::McpConfigureViewController,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let container = NSStackView::new(mtm);
    container.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    container.setSpacing(8.0);
    container.setTranslatesAutoresizingMaskIntoConstraints(false);

    let label = NSTextField::labelWithString(&NSString::from_str("Authentication"), mtm);
    label.setTextColor(Some(&Theme::text_primary()));
    label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    container.addArrangedSubview(&label);

    let auth_popup = unsafe { NSPopUpButton::new(mtm) };
    auth_popup.addItemWithTitle(&NSString::from_str("API Key"));
    auth_popup.addItemWithTitle(&NSString::from_str("Key File"));
    auth_popup.addItemWithTitle(&NSString::from_str("None"));
    unsafe {
        auth_popup.setTarget(Some(controller));
        auth_popup.setAction(Some(sel!(authTypeChanged:)));
    }
    auth_popup.selectItemAtIndex(2);
    let width = auth_popup
        .widthAnchor()
        .constraintGreaterThanOrEqualToConstant(350.0);
    width.setActive(true);
    container.addArrangedSubview(&auth_popup);
    *controller.ivars().auth_type_popup.borrow_mut() = Some(auth_popup);

    let api_key_field = NSTextField::new(mtm);
    api_key_field.setPlaceholderString(Some(&NSString::from_str("Enter API key")));
    api_key_field.setHidden(true);
    let api_key_width = api_key_field
        .widthAnchor()
        .constraintGreaterThanOrEqualToConstant(350.0);
    api_key_width.setActive(true);
    container.addArrangedSubview(&api_key_field);
    *controller.ivars().api_key_input.borrow_mut() = Some(api_key_field);

    let keyfile_field = NSTextField::new(mtm);
    keyfile_field.setPlaceholderString(Some(&NSString::from_str("/path/to/keyfile")));
    keyfile_field.setHidden(true);
    let keyfile_width = keyfile_field
        .widthAnchor()
        .constraintGreaterThanOrEqualToConstant(350.0);
    keyfile_width.setActive(true);
    container.addArrangedSubview(&keyfile_field);
    *controller.ivars().keyfile_input.borrow_mut() = Some(keyfile_field);

    Retained::from(&*container as &NSView)
}

pub fn build_env_var_fields(
    controller: &super::mcp_configure_view::McpConfigureViewController,
    env_vars: &[EnvVarConfig],
    mtm: MainThreadMarker,
) {
    log_to_file(&format!(
        "build_env_var_fields called with {} vars",
        env_vars.len()
    ));

    let form_stack_opt = controller.ivars().form_stack.borrow().clone();
    let form_stack: Retained<super::FlippedStackView> = match form_stack_opt {
        Some(stack) => stack,
        None => return,
    };

    for env_var in env_vars {
        let label_text = if env_var.required {
            format!("{} (required)", env_var.name)
        } else {
            format!("{} (optional)", env_var.name)
        };

        let label = NSTextField::labelWithString(&NSString::from_str(&label_text), mtm);
        label.setTextColor(Some(&Theme::text_primary()));
        label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        form_stack.addArrangedSubview(&label);

        let input = NSTextField::new(mtm);
        input.setPlaceholderString(Some(&NSString::from_str(&env_var.name)));
        input.setTranslatesAutoresizingMaskIntoConstraints(false);

        let var_name_lower = env_var.name.to_lowercase();
        let is_secret = var_name_lower.contains("key")
            || var_name_lower.contains("secret")
            || var_name_lower.contains("token")
            || var_name_lower.contains("password")
            || var_name_lower.contains("pat");

        if is_secret {
            input.setPlaceholderString(Some(&NSString::from_str(&format!(
                "Enter {}",
                env_var.name
            ))));
        }

        let width = input
            .widthAnchor()
            .constraintGreaterThanOrEqualToConstant(350.0);
        width.setActive(true);
        form_stack.addArrangedSubview(&input);

        controller
            .ivars()
            .env_var_inputs
            .borrow_mut()
            .push((env_var.name.clone(), input));

        log_to_file(&format!("Added field for {}", env_var.name));
    }
}

pub fn build_package_args_fields(
    controller: &super::mcp_configure_view::McpConfigureViewController,
    package_args: &[McpPackageArg],
    mtm: MainThreadMarker,
) {
    log_to_file(&format!(
        "build_package_args_fields called with {} args",
        package_args.len()
    ));

    let form_stack_opt = controller.ivars().form_stack.borrow().clone();
    let form_stack: Retained<super::FlippedStackView> = if let Some(stack) = form_stack_opt {
        stack
    } else {
        log_to_file("ERROR: form_stack not available for package args");
        return;
    };

    let title = NSTextField::labelWithString(&NSString::from_str("Package Arguments"), mtm);
    title.setTextColor(Some(&Theme::text_primary()));
    title.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    form_stack.addArrangedSubview(&title);

    for arg in package_args {
        let label_text = if arg.required {
            format!("{} (required)", arg.name)
        } else {
            format!("{} (optional)", arg.name)
        };

        let label = NSTextField::labelWithString(&NSString::from_str(&label_text), mtm);
        label.setTextColor(Some(&Theme::text_primary()));
        label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        form_stack.addArrangedSubview(&label);

        let input = NSTextField::new(mtm);
        let placeholder = arg.default.as_deref().unwrap_or(&arg.name).to_string();
        input.setPlaceholderString(Some(&NSString::from_str(&placeholder)));
        input.setTranslatesAutoresizingMaskIntoConstraints(false);

        if let Some(default_value) = &arg.default {
            input.setStringValue(&NSString::from_str(default_value));
        }

        let width = input
            .widthAnchor()
            .constraintGreaterThanOrEqualToConstant(350.0);
        width.setActive(true);
        form_stack.addArrangedSubview(&input);

        controller
            .ivars()
            .package_arg_inputs
            .borrow_mut()
            .push((arg.name.clone(), input));

        log_to_file(&format!("Added package arg field for {}", arg.name));
    }
}
