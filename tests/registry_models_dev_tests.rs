use personal_agent::registry::ModelsDevClient;

#[tokio::test]
async fn models_dev_client_returns_error_on_bad_status() {
    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let client = ModelsDevClient::with_url(format!("{}/api.json", mock_server.uri()));
    let result = client.fetch_registry().await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("HTTP"));
}
