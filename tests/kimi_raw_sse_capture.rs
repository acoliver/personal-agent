//! Captures the raw SSE stream from Kimi to see the actual format.

#[tokio::test]
#[ignore = "requires PA_E2E_API_KEY or KIMI_API_KEY env var"]
async fn capture_kimi_sse_stream() {
    let api_key = std::env::var("PA_E2E_API_KEY")
        .or_else(|_| std::env::var("KIMI_API_KEY"))
        .expect("Set PA_E2E_API_KEY or KIMI_API_KEY");

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("RooCode/1.0"),
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .expect("client");

    let body = serde_json::json!({
        "model": "kimi-k2-0711-preview",
        "messages": [
            {"role": "user", "content": "Say exactly: pong"}
        ],
        "max_tokens": 200,
        "temperature": 0.0,
        "stream": true
    });

    let response = client
        .post("https://api.kimi.com/coding/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .expect("HTTP request should send");

    let status = response.status();
    println!("Status: {status}");
    println!("Headers: {:#?}", response.headers());

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        panic!("Kimi API returned {status}: {body}");
    }

    // Read the raw SSE stream
    let body_bytes = response.bytes().await.expect("read body");
    let body_text = String::from_utf8_lossy(&body_bytes);

    println!("\n=== RAW SSE STREAM ===");
    for (i, line) in body_text.lines().enumerate() {
        println!("[{i:03}] {line}");
    }
    println!("=== END RAW SSE STREAM ===");
}
