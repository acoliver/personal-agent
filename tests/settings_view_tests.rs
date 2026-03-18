use personal_agent::presentation::view_command::{McpStatus as CommandMcpStatus, ProfileSummary};
use personal_agent::ui_gpui::views::{McpItem, McpStatus, ProfileItem, SettingsState};
use uuid::Uuid;

fn profile_summary(
    id: Uuid,
    name: &str,
    provider_id: &str,
    model_id: &str,
    is_default: bool,
) -> ProfileSummary {
    ProfileSummary {
        id,
        name: name.to_string(),
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        is_default,
    }
}

fn apply_profile_summaries(
    state: &mut SettingsState,
    profiles: Vec<ProfileSummary>,
    selected_profile_id: Option<Uuid>,
) {
    state.profiles = profiles
        .into_iter()
        .map(|profile| {
            ProfileItem::new(profile.id, profile.name)
                .with_model(profile.provider_id, profile.model_id)
                .with_default(profile.is_default)
        })
        .collect();

    if selected_profile_id.is_some() {
        state.selected_profile_id = selected_profile_id;
    }

    if state.selected_profile_id.is_none() {
        state.selected_profile_id = state.profiles.first().map(|profile| profile.id);
    }

    if let Some(selected_id) = state.selected_profile_id {
        if state
            .profiles
            .iter()
            .all(|profile| profile.id != selected_id)
        {
            state.selected_profile_id = state.profiles.first().map(|profile| profile.id);
        }
    }
}

fn set_mcps(state: &mut SettingsState, mcps: Vec<McpItem>) {
    state.mcps = mcps;

    if state.selected_mcp_id.is_none() {
        state.selected_mcp_id = state.mcps.first().map(|mcp| mcp.id);
    }

    if let Some(selected_id) = state.selected_mcp_id {
        if state.mcps.iter().all(|mcp| mcp.id != selected_id) {
            state.selected_mcp_id = state.mcps.first().map(|mcp| mcp.id);
        }
    }
}

fn selected_profile_index(state: &SettingsState) -> Option<usize> {
    state
        .selected_profile_id
        .and_then(|id| state.profiles.iter().position(|profile| profile.id == id))
}

fn select_profile_by_index(state: &mut SettingsState, index: usize) {
    if let Some(profile) = state.profiles.get(index) {
        state.selected_profile_id = Some(profile.id);
    }
}

fn scroll_profiles(state: &mut SettingsState, delta_steps: i32) {
    if state.profiles.is_empty() || delta_steps == 0 {
        return;
    }

    let current = selected_profile_index(state).unwrap_or(0);
    let max_index = state.profiles.len().saturating_sub(1);
    let next = if delta_steps > 0 {
        let positive_steps: usize = delta_steps
            .try_into()
            .expect("positive scroll delta should fit usize");
        current.saturating_add(positive_steps).min(max_index)
    } else {
        let negative_steps: usize = delta_steps.unsigned_abs() as usize;
        current.saturating_sub(negative_steps)
    };
    select_profile_by_index(state, next);
}

fn apply_profile_created(state: &mut SettingsState, id: Uuid, name: &str) {
    state.selected_profile_id = Some(id);
    if state.profiles.iter().all(|profile| profile.id != id) {
        state
            .profiles
            .push(ProfileItem::new(id, name).with_model("", ""));
    }
}

fn apply_profile_updated(state: &mut SettingsState, id: Uuid, name: &str) {
    if let Some(profile) = state.profiles.iter_mut().find(|profile| profile.id == id) {
        profile.name = name.to_string();
    }
}

fn apply_profile_deleted(state: &mut SettingsState, id: Uuid) {
    state.profiles.retain(|profile| profile.id != id);
    if state.selected_profile_id == Some(id) {
        state.selected_profile_id = state.profiles.first().map(|profile| profile.id);
    }
}

fn apply_default_profile_changed(state: &mut SettingsState, profile_id: Option<Uuid>) {
    state.selected_profile_id = profile_id;
    for profile in &mut state.profiles {
        profile.is_default = Some(profile.id) == profile_id;
    }
}

fn apply_mcp_status_changed(state: &mut SettingsState, id: Uuid, status: CommandMcpStatus) {
    let mapped = match status {
        CommandMcpStatus::Running => McpStatus::Running,
        CommandMcpStatus::Failed | CommandMcpStatus::Unhealthy => McpStatus::Error,
        _ => McpStatus::Stopped,
    };

    if let Some(existing) = state.mcps.iter_mut().find(|mcp| mcp.id == id) {
        existing.status = mapped;
        existing.enabled = matches!(mapped, McpStatus::Running);
    } else {
        state
            .mcps
            .push(McpItem::new(id, format!("MCP {id}")).with_status(mapped));
    }
}

fn apply_mcp_server_started(state: &mut SettingsState, id: Uuid) {
    if let Some(existing) = state.mcps.iter_mut().find(|mcp| mcp.id == id) {
        existing.status = McpStatus::Running;
        existing.enabled = true;
    } else {
        state
            .mcps
            .push(McpItem::new(id, format!("MCP {id}")).with_enabled(true));
    }
}

fn apply_mcp_server_failed(state: &mut SettingsState, id: Uuid) {
    if let Some(existing) = state.mcps.iter_mut().find(|mcp| mcp.id == id) {
        existing.status = McpStatus::Error;
        existing.enabled = false;
    } else {
        state
            .mcps
            .push(McpItem::new(id, format!("MCP {id}")).with_status(McpStatus::Error));
    }
}

fn apply_mcp_config_saved(state: &mut SettingsState, id: Uuid, name: Option<&str>) {
    state.selected_mcp_id = Some(id);
    if let Some(existing) = state.mcps.iter_mut().find(|mcp| mcp.id == id) {
        if let Some(name) = name {
            existing.name = name.to_string();
        }
        existing.enabled = true;
        existing.status = McpStatus::Running;
    } else {
        state.mcps.push(
            McpItem::new(
                id,
                name.map_or_else(|| format!("MCP {id}"), ToString::to_string),
            )
            .with_status(McpStatus::Running)
            .with_enabled(true),
        );
    }
}

fn apply_mcp_deleted(state: &mut SettingsState, id: Uuid) {
    state.mcps.retain(|mcp| mcp.id != id);
    if state.selected_mcp_id == Some(id) {
        state.selected_mcp_id = state.mcps.first().map(|mcp| mcp.id);
    }
}

#[test]
fn profile_item_builders_and_display_text_cover_real_output() {
    let id = Uuid::new_v4();

    let with_model = ProfileItem::new(id, "Primary")
        .with_model("openai", "gpt-4o")
        .with_default(true);
    let bare = ProfileItem::new(id, "Bare");

    assert_eq!(with_model.id, id);
    assert_eq!(with_model.provider, "openai");
    assert_eq!(with_model.model, "gpt-4o");
    assert!(with_model.is_default);
    assert_eq!(with_model.display_text(), "Primary (openai:gpt-4o)");
    assert_eq!(bare.display_text(), "Bare");
}

#[test]
fn mcp_item_builders_map_enabled_and_status_as_expected() {
    let id = Uuid::new_v4();

    let running = McpItem::new(id, "Fetch").with_enabled(true);
    let stopped = McpItem::new(id, "Fetch").with_enabled(false);
    let error = McpItem::new(id, "Fetch").with_status(McpStatus::Error);

    assert!(running.enabled);
    assert_eq!(running.status, McpStatus::Running);
    assert!(!stopped.enabled);
    assert_eq!(stopped.status, McpStatus::Stopped);
    assert!(!error.enabled);
    assert_eq!(error.status, McpStatus::Error);
}

#[test]
fn settings_state_new_uses_expected_defaults() {
    let state = SettingsState::new();

    assert!(state.profiles.is_empty());
    assert!(state.mcps.is_empty());
    assert_eq!(state.selected_profile_id, None);
    assert_eq!(state.selected_mcp_id, None);
    assert_eq!(state.hotkey, "Cmd+Shift+P");
}

#[test]
fn profile_summary_application_selects_first_and_falls_back_when_selection_disappears() {
    let profile_a = Uuid::new_v4();
    let profile_b = Uuid::new_v4();
    let profile_c = Uuid::new_v4();
    let mut state = SettingsState::new();

    apply_profile_summaries(
        &mut state,
        vec![
            profile_summary(profile_a, "Alpha", "openai", "gpt-4o", false),
            profile_summary(profile_b, "Beta", "anthropic", "claude", true),
        ],
        None,
    );
    assert_eq!(state.selected_profile_id, Some(profile_a));
    assert_eq!(state.profiles[0].display_text(), "Alpha (openai:gpt-4o)");

    apply_profile_summaries(
        &mut state,
        vec![
            profile_summary(profile_b, "Beta", "anthropic", "claude", true),
            profile_summary(profile_c, "Gamma", "openai", "gpt-4.1", false),
        ],
        Some(profile_b),
    );
    assert_eq!(state.selected_profile_id, Some(profile_b));

    apply_profile_summaries(
        &mut state,
        vec![profile_summary(
            profile_c, "Gamma", "openai", "gpt-4.1", false,
        )],
        Some(profile_b),
    );
    assert_eq!(state.selected_profile_id, Some(profile_c));
}

#[test]
fn profile_selection_helpers_and_commands_cover_create_update_delete_default_and_ignore() {
    let profile_a = Uuid::new_v4();
    let profile_b = Uuid::new_v4();
    let profile_created = Uuid::new_v4();
    let mut state = SettingsState::new();

    state.profiles = vec![
        ProfileItem::new(profile_a, "Alpha").with_model("openai", "gpt-4o"),
        ProfileItem::new(profile_b, "Beta").with_model("anthropic", "claude"),
    ];
    state.selected_profile_id = Some(profile_a);

    assert_eq!(selected_profile_index(&state), Some(0));
    scroll_profiles(&mut state, 1);
    assert_eq!(state.selected_profile_id, Some(profile_b));
    scroll_profiles(&mut state, 10);
    assert_eq!(state.selected_profile_id, Some(profile_b));
    scroll_profiles(&mut state, -10);
    assert_eq!(state.selected_profile_id, Some(profile_a));

    apply_profile_created(&mut state, profile_created, "Created");
    assert_eq!(state.selected_profile_id, Some(profile_created));
    assert!(state
        .profiles
        .iter()
        .any(|profile| profile.id == profile_created && profile.name == "Created"));

    let profiles_after_create = state.profiles.len();
    apply_profile_created(&mut state, profile_created, "Created Again");
    assert_eq!(state.profiles.len(), profiles_after_create);

    apply_profile_updated(&mut state, profile_created, "Renamed Profile");
    assert_eq!(
        state
            .profiles
            .iter()
            .find(|profile| profile.id == profile_created)
            .expect("created profile exists")
            .name,
        "Renamed Profile"
    );

    apply_default_profile_changed(&mut state, Some(profile_b));
    assert_eq!(state.selected_profile_id, Some(profile_b));
    assert!(
        state
            .profiles
            .iter()
            .find(|profile| profile.id == profile_b)
            .expect("profile_b exists")
            .is_default
    );
    assert_eq!(
        state
            .profiles
            .iter()
            .filter(|profile| profile.is_default)
            .count(),
        1
    );

    let unchanged_profiles = state.profiles.clone();
    let unchanged_selection = state.selected_profile_id;
    assert_eq!(state.profiles, unchanged_profiles);
    assert_eq!(state.selected_profile_id, unchanged_selection);

    apply_profile_deleted(&mut state, profile_b);
    assert_eq!(state.selected_profile_id, Some(profile_a));
    assert!(state.profiles.iter().all(|profile| profile.id != profile_b));
}

#[test]
fn mcp_commands_and_selection_fallback_cover_real_status_logic() {
    let mcp_existing = Uuid::new_v4();
    let mcp_new = Uuid::new_v4();
    let mcp_saved = Uuid::new_v4();
    let mut state = SettingsState::new();

    set_mcps(
        &mut state,
        vec![
            McpItem::new(mcp_existing, "Existing").with_status(McpStatus::Stopped),
            McpItem::new(mcp_new, "Second").with_enabled(true),
        ],
    );
    assert_eq!(state.selected_mcp_id, Some(mcp_existing));

    state.selected_mcp_id = Some(Uuid::new_v4());
    set_mcps(
        &mut state,
        vec![McpItem::new(mcp_new, "Second").with_enabled(true)],
    );
    assert_eq!(state.selected_mcp_id, Some(mcp_new));

    apply_mcp_status_changed(&mut state, mcp_new, CommandMcpStatus::Failed);
    let mcp = state
        .mcps
        .iter()
        .find(|mcp| mcp.id == mcp_new)
        .expect("existing mcp updated");
    assert_eq!(mcp.status, McpStatus::Error);
    assert!(!mcp.enabled);

    apply_mcp_status_changed(&mut state, mcp_existing, CommandMcpStatus::Running);
    let mcp = state
        .mcps
        .iter()
        .find(|mcp| mcp.id == mcp_existing)
        .expect("existing mcp updated");
    assert_eq!(mcp.status, McpStatus::Running);
    assert!(!mcp.enabled);

    apply_mcp_server_started(&mut state, mcp_saved);
    let started = state
        .mcps
        .iter()
        .find(|mcp| mcp.id == mcp_saved)
        .expect("new started mcp inserted");
    assert_eq!(started.name, format!("MCP {mcp_saved}"));
    assert_eq!(started.status, McpStatus::Running);
    assert!(started.enabled);

    apply_mcp_server_failed(&mut state, mcp_saved);
    let failed = state
        .mcps
        .iter()
        .find(|mcp| mcp.id == mcp_saved)
        .expect("started mcp exists");
    assert_eq!(failed.status, McpStatus::Error);
    assert!(!failed.enabled);

    apply_mcp_config_saved(&mut state, mcp_saved, Some("Saved MCP"));
    let saved = state
        .mcps
        .iter()
        .find(|mcp| mcp.id == mcp_saved)
        .expect("saved mcp exists");
    assert_eq!(state.selected_mcp_id, Some(mcp_saved));
    assert_eq!(saved.name, "Saved MCP");
    assert_eq!(saved.status, McpStatus::Running);
    assert!(saved.enabled);

    apply_mcp_deleted(&mut state, mcp_saved);
    assert!(state.mcps.iter().all(|mcp| mcp.id != mcp_saved));
    assert_eq!(state.selected_mcp_id, Some(mcp_new));

    let unchanged_mcps = state.mcps.clone();
    let unchanged_selection = state.selected_mcp_id;
    assert_eq!(state.mcps, unchanged_mcps);
    assert_eq!(state.selected_mcp_id, unchanged_selection);
}
