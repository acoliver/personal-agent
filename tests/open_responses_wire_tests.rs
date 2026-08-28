//! What `PersonalAgent` actually puts on the Responses websocket, and what it
//! makes of what comes back.
//!
//! These run against a raw `tokio-tungstenite` peer on loopback. No network,
//! no credentials, and the frames are written as literal JSON so a change in
//! the wire shape fails here rather than in production.

use std::time::Duration;

use futures::{SinkExt, StreamExt};
use personal_agent::llm::{LlmClient, Message, StreamEvent};
use personal_agent::models::{AuthConfig, ModelProfile};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{accept_async, WebSocketStream};
use uuid::Uuid;

type Peer = WebSocketStream<TcpStream>;

/// Accept one websocket connection.
async fn accept(listener: &TcpListener) -> Peer {
    let (stream, _) = listener.accept().await.expect("accept");
    accept_async(stream).await.expect("websocket upgrade")
}

/// Read one turn frame from the client.
async fn read_frame(peer: &mut Peer) -> Value {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), peer.next())
            .await
            .expect("timed out waiting for a turn frame")
            .expect("socket closed before a turn arrived")
            .expect("socket error");
        match message {
            WsMessage::Text(text) => {
                return serde_json::from_str(&text).expect("turn frame is JSON")
            }
            WsMessage::Close(_) => panic!("client closed before sending a turn"),
            // Pings and binary frames are noise between turns.
            _ => {}
        }
    }
}

async fn send(peer: &mut Peer, event: &Value) {
    peer.send(WsMessage::Text(event.to_string()))
        .await
        .expect("send event");
}

/// A response object shaped the way the backend sends it, with one assistant
/// message and usage.
fn completed_response(id: &str, model: &str, text: &str) -> Value {
    json!({
        "id": id,
        "object": "response",
        "created_at": 0,
        "status": "completed",
        "model": model,
        "output": [{
            "type": "message",
            "id": format!("msg_{id}"),
            "role": "assistant",
            "status": "completed",
            "content": [{"type": "output_text", "text": text, "annotations": []}]
        }],
        "usage": {"input_tokens": 11, "output_tokens": 7, "total_tokens": 18}
    })
}

/// Stream one complete turn: item added, text delta, item done, completed.
async fn stream_turn(peer: &mut Peer, id: &str, model: &str, text: &str) {
    send(
        peer,
        &json!({
            "type": "response.output_item.added",
            "sequence_number": 1,
            "output_index": 0,
            "item": {
                "type": "message",
                "id": format!("msg_{id}"),
                "role": "assistant",
                "status": "in_progress",
                "content": []
            }
        }),
    )
    .await;
    send(
        peer,
        &json!({
            "type": "response.output_text.delta",
            "sequence_number": 2,
            "item_id": format!("msg_{id}"),
            "output_index": 0,
            "content_index": 0,
            "delta": text
        }),
    )
    .await;
    send(
        peer,
        &json!({
            "type": "response.output_item.done",
            "sequence_number": 3,
            "output_index": 0,
            "item": {
                "type": "message",
                "id": format!("msg_{id}"),
                "role": "assistant",
                "status": "completed",
                "content": [{"type": "output_text", "text": text, "annotations": []}]
            }
        }),
    )
    .await;
    send(
        peer,
        &json!({
            "type": "response.completed",
            "sequence_number": 4,
            "response": completed_response(id, model, text)
        }),
    )
    .await;
}

fn profile(endpoint: &str) -> ModelProfile {
    ModelProfile::new(
        "Wire".to_string(),
        "open-responses".to_string(),
        "gpt-5.6-luna".to_string(),
        endpoint.to_string(),
        AuthConfig::None,
    )
}

fn user(text: &str) -> Message {
    Message::user(text)
}

/// Drain a streaming turn into the events `PersonalAgent` hands the UI.
async fn run_turn(client: &LlmClient, history: &[Message]) -> Vec<StreamEvent> {
    let collected = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&collected);
    client
        .request_stream(history, move |event| {
            sink.lock().expect("event sink").push(event);
        })
        .await
        .expect("stream");
    let events = collected.lock().expect("event sink").clone();
    events
}

#[tokio::test]
async fn the_turn_frame_is_flat_and_carries_no_response_wrapper() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        let mut peer = accept(&listener).await;
        let frame = read_frame(&mut peer).await;
        stream_turn(&mut peer, "resp_1", "gpt-5.6-luna", "ok").await;
        frame
    });

    let client = LlmClient::from_profile(&profile(&format!("ws://{addr}/v1/responses")))
        .expect("client")
        .for_conversation(Uuid::new_v4());
    run_turn(&client, &[user("hello")]).await;

    let frame = server.await.expect("server task");
    assert_eq!(frame["type"], "response.create");
    assert_eq!(
        frame["model"], "gpt-5.6-luna",
        "the model belongs on the frame root; nested it parses as None"
    );
    assert!(
        frame.get("response").is_none(),
        "the codex frame is flat, not wrapped: {frame}"
    );
    assert!(frame.get("input").is_some(), "frame carries input: {frame}");
}

#[tokio::test]
async fn a_chained_turn_sends_only_the_new_input() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        let mut peer = accept(&listener).await;
        let first = read_frame(&mut peer).await;
        stream_turn(&mut peer, "resp_1", "gpt-5.6-luna", "first").await;
        let second = read_frame(&mut peer).await;
        stream_turn(&mut peer, "resp_2", "gpt-5.6-luna", "second").await;
        (first, second)
    });

    let client = LlmClient::from_profile(&profile(&format!("ws://{addr}/v1/responses")))
        .expect("client")
        .for_conversation(Uuid::new_v4());

    let history = vec![user("first question")];
    run_turn(&client, &history).await;

    let history = vec![
        user("first question"),
        Message::assistant("first"),
        user("second question"),
    ];
    run_turn(&client, &history).await;

    let (first, second) = server.await.expect("server task");

    assert!(
        first["previous_response_id"].is_null(),
        "the opening turn has nothing to chain onto: {first}"
    );
    assert_eq!(
        second["previous_response_id"], "resp_1",
        "the second turn chains onto the first: {second}"
    );

    let first_items = first["input"].as_array().expect("input array").len();
    let second_items = second["input"].as_array().expect("input array").len();
    assert!(
        second_items < first_items + 2,
        "the chained turn resent the history: first={first_items} second={second_items}"
    );
    assert_eq!(
        second_items, 1,
        "only the new user item goes out on a chained turn: {second}"
    );
}

#[tokio::test]
async fn text_deltas_and_usage_reach_the_ui() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let mut peer = accept(&listener).await;
        let _ = read_frame(&mut peer).await;
        stream_turn(&mut peer, "resp_1", "gpt-5.6-luna", "haiku").await;
    });

    let client = LlmClient::from_profile(&profile(&format!("ws://{addr}/v1/responses")))
        .expect("client")
        .for_conversation(Uuid::new_v4());
    let events = run_turn(&client, &[user("hello")]).await;

    let text: String = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::TextDelta(delta) => Some(delta.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "haiku");

    let completions: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::Complete {
                input_tokens,
                output_tokens,
            } => Some((*input_tokens, *output_tokens)),
            _ => None,
        })
        .collect();
    assert_eq!(
        completions.len(),
        1,
        "exactly one terminal event: {events:?}"
    );
    assert_eq!(
        completions[0],
        (Some(11), Some(7)),
        "usage from the provider reaches the UI"
    );
}

#[tokio::test]
async fn reasoning_summary_deltas_arrive_as_thinking() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let mut peer = accept(&listener).await;
        let _ = read_frame(&mut peer).await;
        send(
            &mut peer,
            &json!({
                "type": "response.output_item.added",
                "sequence_number": 1,
                "output_index": 0,
                "item": {
                    "type": "reasoning",
                    "id": "rs_1",
                    "summary": []
                }
            }),
        )
        .await;
        send(
            &mut peer,
            &json!({
                "type": "response.reasoning_summary_text.delta",
                "sequence_number": 2,
                "item_id": "rs_1",
                "output_index": 0,
                "summary_index": 0,
                "delta": "weighing it up"
            }),
        )
        .await;
        stream_turn(&mut peer, "resp_1", "gpt-5.6-luna", "done").await;
    });

    let client = LlmClient::from_profile(&profile(&format!("ws://{addr}/v1/responses")))
        .expect("client")
        .for_conversation(Uuid::new_v4());
    let events = run_turn(&client, &[user("think")]).await;

    let thinking: String = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::ThinkingDelta(delta) => Some(delta.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(thinking, "weighing it up");
}

#[tokio::test]
async fn an_unknown_frame_does_not_end_the_stream() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let mut peer = accept(&listener).await;
        let _ = read_frame(&mut peer).await;
        // The codex backend interleaves quota frames the client has no
        // meaning for. Skipping them is the contract.
        send(
            &mut peer,
            &json!({
                "type": "codex.rate_limits",
                "sequence_number": 0,
                "primary": {"used_percent": 12.5}
            }),
        )
        .await;
        stream_turn(&mut peer, "resp_1", "gpt-5.6-luna", "still here").await;
    });

    let client = LlmClient::from_profile(&profile(&format!("ws://{addr}/v1/responses")))
        .expect("client")
        .for_conversation(Uuid::new_v4());
    let events = run_turn(&client, &[user("hello")]).await;

    let text: String = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::TextDelta(delta) => Some(delta.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "still here");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, StreamEvent::Error(_))),
        "an unknown frame is skipped, not surfaced: {events:?}"
    );
}

#[tokio::test]
async fn two_conversations_do_not_share_one_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        let mut first = accept(&listener).await;
        let _ = read_frame(&mut first).await;
        stream_turn(&mut first, "resp_a", "gpt-5.6-luna", "a").await;

        // A second conversation has to dial its own connection; if it shared
        // the first socket this accept would never complete.
        let mut second = accept(&listener).await;
        let frame = read_frame(&mut second).await;
        stream_turn(&mut second, "resp_b", "gpt-5.6-luna", "b").await;
        frame
    });

    let endpoint = format!("ws://{addr}/v1/responses");
    let first = LlmClient::from_profile(&profile(&endpoint))
        .expect("client")
        .for_conversation(Uuid::new_v4());
    run_turn(&first, &[user("one")]).await;

    let second = LlmClient::from_profile(&profile(&endpoint))
        .expect("client")
        .for_conversation(Uuid::new_v4());
    run_turn(&second, &[user("two")]).await;

    let frame = tokio::time::timeout(Duration::from_secs(10), server)
        .await
        .expect("the second conversation opened its own socket")
        .expect("server task");
    assert!(
        frame["previous_response_id"].is_null(),
        "a fresh conversation starts an unchained turn: {frame}"
    );
}

/// A profile with thinking on, which should put a reasoning block on the wire.
fn thinking_profile(endpoint: &str, budget: u32) -> ModelProfile {
    let mut profile = profile(endpoint);
    profile.parameters.enable_thinking = true;
    profile.parameters.thinking_budget = Some(budget);
    profile
}

#[tokio::test]
async fn thinking_puts_a_reasoning_block_on_the_wire() {
    // A live turn against the real backend came back with zero reasoning
    // tokens, which leaves two possibilities: the model declined, or the
    // request never asked. This settles which, without a network.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");

    let server = tokio::spawn(async move {
        let mut peer = accept(&listener).await;
        let frame = read_frame(&mut peer).await;
        stream_turn(&mut peer, "resp_1", "gpt-5.6-luna", "ok").await;
        frame
    });

    let client = LlmClient::from_profile(&thinking_profile(
        &format!("ws://{addr}/v1/responses"),
        20_000,
    ))
    .expect("client");
    let _ = run_turn(&client, &[user("think about it")]).await;

    let frame = server.await.expect("server");
    let reasoning = frame
        .get("reasoning")
        .unwrap_or_else(|| panic!("no reasoning block in frame: {frame}"));

    assert_eq!(
        reasoning.get("effort").and_then(Value::as_str),
        Some("high"),
        "frame was {frame}"
    );
    assert_eq!(
        reasoning.get("summary").and_then(Value::as_str),
        Some("auto"),
        "a summary must be requested or the backend sends no reasoning deltas; frame was {frame}"
    );
}

#[tokio::test]
async fn thinking_off_sends_no_reasoning_block() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");

    let server = tokio::spawn(async move {
        let mut peer = accept(&listener).await;
        let frame = read_frame(&mut peer).await;
        stream_turn(&mut peer, "resp_1", "gpt-5.6-luna", "ok").await;
        frame
    });

    let client =
        LlmClient::from_profile(&profile(&format!("ws://{addr}/v1/responses"))).expect("client");
    let _ = run_turn(&client, &[user("hi")]).await;

    let frame = server.await.expect("server");
    assert!(frame.get("reasoning").is_none(), "frame was {frame}");
}
