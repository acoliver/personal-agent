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
| `is_placeholder_title`           | `None` / blank / `"New Conversation"` / `"Untitled…"` / the 17-digit `Conversation::new` timestamp default |
| `build_title_request_messages`   | System + user messages, first prompt truncated to `MAX_PROMPT_CHARS` |
| `sanitize_generated_title`       | Thinking-block strip -> first non-empty line -> decoration/label/quote strip -> whitespace collapse -> trailing punctuation strip -> reject empty -> cap at `MAX_TITLE_CHARS` on a word boundary |

`sanitize_generated_title` is applied inside `LlmConversationTitleGenerator`, so the trait
contract is "returns a usable title or an error".

### New module: `src/services/chat_impl/titling.rs`

`TitleGenerationRequest::for_first_prompt(conversation, profile)` returns `Some` only when:

1. the conversation title is a placeholder, and
2. the conversation contains exactly one user message.

`generate_and_apply_title(...)`:

1. `tokio::time::timeout(TITLE_GENERATION_TIMEOUT, generator.generate_title(..))`
2. reload the conversation and re-check the title is still a placeholder — the user may
   have renamed it while the request was in flight; if so, skip (do not clobber)
3. `conversation_service.rename(id, title)`
4. `emit(AppEvent::Conversation(ConversationEvent::TitleUpdated { id, title }))`

Every failure path logs and returns; nothing propagates to the caller.

### Wiring in `ChatServiceImpl`

- New field `title_generator: Arc<dyn ConversationTitleGenerator>`, defaulted in `new()` to
  `LlmConversationTitleGenerator`. Public constructor signatures are unchanged; tests inject
  a fake through the builder `with_title_generator(..)`.
- `prepare_message_context` computes `title_request: Option<TitleGenerationRequest>` from the
  conversation it already reloads after persisting the user message.
- `send_message` takes the request out of the prepared context and, after the stream task is
  spawned, spawns an independent `tokio` task for title generation.

### UI propagation (already exists, no new plumbing)

`ConversationEvent::TitleUpdated` -> `ChatPresenter::handle_conversation_event` ->
`ViewCommand::ConversationRenamed` -> `AppStore::reduce_conversation_renamed` ->
`update_conversation_title` (history list + selected title). Verified store-managed in
`is_store_managed`.

## Test strategy

Unit (`src/services/conversation_title.rs`):

- placeholder detection for each placeholder shape and for real user titles
- prompt truncation and message shape
- sanitization: quotes, markdown bold/heading/bullet, `Title:` label, `<think>` blocks
  (terminated and unterminated), multi-line preamble, trailing punctuation, whitespace
  collapse, over-long titles, empty/whitespace-only rejection, multi-byte safety

Behavioural (`src/services/chat_impl/tests/title_generation.rs`), using a recording
`MockConversationService` and a scripted generator:

- untitled + first prompt -> `rename` persisted with the sanitized title **and**
  `ConversationEvent::TitleUpdated` published
- generator error -> no rename, placeholder preserved
- generator hang -> timeout, no rename
- user renamed while generating -> no clobber
- `for_first_prompt` gating: already-titled, second prompt, no user message

`MockConversationService` gains a real title cell so `rename` is observable through `load`,
which is what makes the no-clobber test meaningful rather than mock theater.

## Verification

`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test --lib --tests`, structural/lizard checks.

## Out of scope

- Setting to disable auto-naming.
- Retro-titling existing conversations.
- Changes to manual rename.
