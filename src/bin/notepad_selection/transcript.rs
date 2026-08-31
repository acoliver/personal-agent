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
            r"### Streaming selection matrix

- `response.output_text.delta` appends **visible text**
- `response.reasoning.delta` carries thinking
- `response.completed` closes the run

```rust
while let Some(delta) = socket.next().await {
    buffer.push_str(delta?);
}
```

> Apply deltas in arrival order.

| event | timeout |
|:--|--:|
| `response.created` | 15s |
| `response.output_text.delta` | 90s |
| `response.completed` | none |

Read the [Responses WebSocket guide](https://platform.openai.com/docs/guides/realtime-websocket).",
        ),
        (Role::User, "Got it. Thanks."),
    ]
}
