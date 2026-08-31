#[derive(Clone, Copy)]
pub enum Role {
    User,
    Assistant,
}

pub fn transcript() -> Vec<(Role, &'static str)> {
    vec![
        (
            Role::User,
            "How do I stream tokens from the Responses websocket?",
        ),
        (
            Role::Assistant,
            r"### Streaming over the Responses socket

Open the socket first, then send the request frame. The server replies with a `response.created` event, followed by a run of delta events. Each delta carries a fragment of the assistant message; concatenate them in arrival order.

- `response.output_text.delta` carries the visible text
- `response.reasoning.delta` carries thinking, if enabled
- `response.completed` closes the run

```rust
while let Some(frame) = socket.next().await {
    match parse(frame)? {
        Event::Delta(text) => buffer.push_str(&text),
        Event::Completed => break,
    }
}
```

> Apply deltas in arrival order. Reordering fragments corrupts the visible response.

Full protocol notes are in the [Responses WebSocket guide](https://platform.openai.com/docs/guides/realtime-websocket).

---

![socket event timeline](https://example.com/socket-event-timeline.png)",
        ),
        (Role::User, "And what closes it if the model stalls?"),
        (
            Role::Assistant,
            r"An idle timer. If no frame arrives for the configured window, the client sends a close frame and surfaces a timeout to the caller, leaving whatever text had already been buffered.

| setting | default |
|:--|--:|
| `idle_timeout` | 90s |
| `connect_timeout` | 15s |

> Preserve the buffered text when reporting the timeout so the caller can decide whether to retry.",
        ),
        (Role::User, "Got it. Thanks."),
    ]
}
