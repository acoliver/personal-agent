# Issue #210 — Auto-generate conversation titles from the first prompt

Replace the permanent `"New Conversation"` placeholder with a model-generated title
derived from the user's first prompt.

## Goal

When a user sends the first prompt into a conversation that is still untitled, ask the
model (via the conversation's own profile) for a short title, persist it, and push it to
the UI. Never block or break the chat stream to do it.

## Acceptance criteria (from issue)

- [ ] First prompt in an untitled conversation triggers a title generation request.
- [ ] Title is persisted via `ConversationService::rename` and reaches the UI (title bar
      and conversation list) without a full reload.
- [ ] Generation is off the critical path: failures/timeouts/junk leave the placeholder
      intact and only log.
- [ ] Already-titled conversations and conversations with prior messages are untouched.
- [ ] Generated titles are sanitized: single line, no markdown/quotes/label prefix, no
      thinking blocks, no trailing sentence punctuation, length-capped, empty rejected.
- [ ] The prompt sent to the model is truncated so a huge first message is not resent whole.
- [ ] Exactly one attempt per conversation.
- [ ] Covered by tests using a mockable generator seam; no live LLM call in unit tests.

## Design

### New module: `src/services/conversation_title.rs`

Pure, unit-testable helpers plus a boundary trait:

```rust
pub trait ConversationTitleGenerator: Send + Sync {
    async fn generate_title(&self, profile: &ModelProfile, first_prompt: &str)
        -> ServiceResult<String>;
}

pub struct LlmConversationTitleGenerator;   // real implementation, uses LlmClient::request
```

Helpers:

| Function                         | Responsibility                                              |
| -------------------------------- | ----------------------------------------------------------- |
| `is_placeholder_title`           | `None` / blank / `"New Conversation"` / `"Untitled…"` — the only titles production storage ever holds for an unnamed conversation |
| `build_title_request_messages`   | System + user messages, first prompt truncated to `MAX_PROMPT_CHARS` |
| `sanitize_generated_title`       | Thinking-block strip -> first non-empty line -> decoration/label/quote strip -> whitespace collapse -> trailing punctuation strip -> reject empty -> cap at `MAX_TITLE_CHARS` on a word boundary |

### New module: `src/services/chat_impl/titling.rs`

`TitleGenerationRequest::for_untitled_conversation(conversation, profile)` returns `Some`
whenever the stored title is still a placeholder, using the conversation's **first** user
message as the prompt. Gating on the title alone rather than on "this is the first send"
matters: the user message is persisted before anything can fail, so a first attempt lost
to a bad key or a dropped connection would otherwise leave the conversation named
"New Conversation" forever.

`generate_and_apply_title(...)`:

1. `generator.propose_title(..)`, bounded by `TITLE_GENERATION_TIMEOUT` and by the turn's
   `CancellationToken` — pressing Stop abandons the title request too
2. `sanitize_generated_title(..)`; `None` means nothing usable, so keep the placeholder
3. reload the conversation and re-check the title is still a placeholder — the user may
   have renamed it while the request was in flight; if so, skip (do not clobber). A
   deleted conversation (`NotFound`) is a normal outcome here, not a warning
4. `conversation_service.rename(id, title)`
5. `view_tx.send(ViewCommand::ConversationTitleUpdated { id, title })`

Every failure path logs and returns; nothing propagates to the caller.

### Wiring in `ChatServiceImpl`

- New field `title_generator: Arc<dyn ConversationTitleGenerator>`, defaulted in `new()` to
  `LlmConversationTitleGenerator`. Public constructor signatures are unchanged; tests inject
  a fake through the builder `with_title_generator(..)`.
- `prepare_message_context` returns `(PreparedMessageContext, Option<TitleGenerationRequest>)`,
  computed from the conversation it already reloads after persisting the user message.
- `send_message` spawns an independent `tokio` task for title generation after the stream
  task, sharing the turn's cancellation token.

### UI propagation

The title update goes out on the service's existing `view_tx`, **not** on the global event
bus. The bus is a 16-slot `broadcast` that also carries one message per streamed token, and
both presenters treat `RecvError::Lagged` as normal and drop what they missed — a one-shot
title update cannot survive that. `view_tx` is the reliable mpsc(256) -> flume(1024) queue
the service already uses for its other UI updates.

`ViewCommand::ConversationTitleUpdated` -> `AppStore::reduce_conversation_title_updated` ->
`update_conversation_title`, which updates the history list, the chat sidebar list, and the
chat title bar. Already store-managed in `is_store_managed`; no new plumbing.

## Test strategy

Unit (`src/services/conversation_title.rs`):

- placeholder detection for each placeholder shape and for real user titles (including an
  all-digit title, which is a legitimate user choice)
- prompt truncation and message shape
- sanitization: quotes, markdown bold/heading/bullet, `Title:` label, `<think>` blocks
  (terminated and unterminated), multi-line preamble, trailing punctuation, whitespace
  collapse, over-long titles, empty/whitespace-only rejection, multi-byte safety

Behavioural (`src/services/chat_impl/tests/title_generation.rs`), using a recording
`MockConversationService` and a scripted generator:

- untitled + first prompt -> `rename` persisted with the sanitized title **and**
  `ViewCommand::ConversationTitleUpdated` published
- that same command fed through the real `GpuiAppStore` updates the chat title bar, the
  history list and the chat sidebar list
- generator error / unusable output -> no rename, placeholder preserved
- generator hang -> timeout (paused clock), no rename
- user renamed while generating -> no clobber, using a gate that holds the proposal open so
  the rename really does land mid-flight
- turn cancelled while generating -> no rename
- conversation deleted while generating -> no rename
- gating: already-titled conversation, no user message, blank prompt, and a still-untitled
  conversation retrying from its original first prompt
- `send_message` end-to-end: untitled conversation is renamed, titled one is not asked at all

`MockConversationService` gains a real title cell so `rename` is observable through `load`,
which is what makes the no-clobber test meaningful rather than mock theater.

## Verification

`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test --lib --tests`, structural/lizard checks.

## Deliberately not done

- **Compare-and-swap rename.** The re-read before `rename` narrows the window to two
  consecutive service calls but is not atomic; making it so would mean a conditional-rename
  operation across `ConversationService` and every implementation. Documented in
  `titling.rs` instead.
- **Collapsing `ConversationRenamed` / `ConversationTitleUpdated`.** They reduce
  identically, but merging them touches every producer and is unrelated to this issue.

## Out of scope

- Setting to disable auto-naming.
- Retro-titling existing conversations.
- Changes to manual rename.
