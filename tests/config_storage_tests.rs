use personal_agent::{Config, Conversation, ConversationStorage, Message, Result};
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn config_load_creates_default_and_persists_updates() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let mut config = Config::load(&config_path)?;
    assert!(config_path.exists());
    assert!(config.profiles.is_empty());

    let profile = personal_agent::ModelProfile::default();
    let profile_id = profile.id;
    config.add_profile(profile);
    config.default_profile = Some(profile_id);
    config.save(&config_path)?;

    let reloaded = Config::load(&config_path)?;
    assert_eq!(reloaded.default_profile, Some(profile_id));
    assert_eq!(reloaded.profiles.len(), 1);

    Ok(())
}

#[test]
fn conversation_storage_round_trip_and_list_order() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let storage = ConversationStorage::new(temp_dir.path());

    let mut conversation = Conversation::new(Uuid::new_v4());
    conversation.add_message(Message::user("Hello".to_string()));
    storage.save(&conversation)?;

    let filename = conversation.filename();
    let loaded = storage.load(&filename)?;
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].content, "Hello");

    std::thread::sleep(std::time::Duration::from_millis(2));

    let mut second = Conversation::new(Uuid::new_v4());
    second.add_message(Message::assistant("Hi".to_string()));
    storage.save(&second)?;

    let list = storage.list()?;
    assert_eq!(list.len(), 2);
    assert!(list[0] >= list[1]);

    Ok(())
}
