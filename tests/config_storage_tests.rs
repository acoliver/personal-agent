use personal_agent::{Config, Message, Result};
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

#[tokio::test]
async fn sqlite_conversation_round_trip_and_list_order() {
    use personal_agent::db::spawn_db_thread;
    use personal_agent::services::{ConversationService, SqliteConversationService};

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    // spawn_db_thread calls blocking_recv internally; use spawn_blocking so it
    // runs off the tokio worker thread and avoids a "cannot block" panic.
    let db = tokio::task::spawn_blocking(move || spawn_db_thread(&db_path).unwrap())
        .await
        .expect("spawn_blocking failed");
    let service = SqliteConversationService::new(db);

    let profile_id = Uuid::new_v4();
    let conv = service
        .create(None, profile_id)
        .await
        .expect("create conversation");
    service
        .add_message(conv.id, Message::user("Hello".to_string()))
        .await
        .expect("add message");

    let loaded = service.load(conv.id).await.expect("load conversation");
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].content, "Hello");

    let second = service
        .create(None, profile_id)
        .await
        .expect("create second");
    service
        .add_message(second.id, Message::assistant("Hi".to_string()))
        .await
        .expect("add assistant message");

    let list = service
        .list_metadata(None, None)
        .await
        .expect("list metadata");
    assert_eq!(list.len(), 2);
    // Most recently updated first
    assert!(list[0].updated_at >= list[1].updated_at);
}
