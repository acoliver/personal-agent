# Pseudocode 02: Conversation Selection Loading Protocol

## Overview

Selection must be explicit, generation-aware, and independent of popup timing.

## Selection Protocol (Lines 1-109)

```text
001  FUNCTION ChatView_or_HistoryView_on_select(conversation_id):
002    emit raw UI intent to gpui_runtime.handle_select_conversation_intent(conversation_id)
003
004  FUNCTION handle_select_conversation_intent(conversation_id):
005    LET selection_result = gpui_store_boundary.begin_selection(conversation_id)
006    MATCH selection_result:
007      NoOpSameSelection =>
008        RETURN without publication or async load dispatch
009      BeganSelection { generation } =>
010        emit UserEvent::SelectConversation {
011          id: conversation_id,
012          selection_generation: generation,
013        }
014        RETURN
015
016  FUNCTION begin_selection(conversation_id):
017    IF selected_conversation_id == Some(conversation_id)
018      AND load_state is Loading/Ready for current selection_generation:
019        RETURN NoOpSameSelection
020    IF selected_conversation_id == Some(conversation_id)
021      AND load_state is Error for current selection_generation:
022        LET next_generation = current selection_generation + 1
023        selected_conversation_id = Some(conversation_id)
024        selected_title = lookup_title_or_fallback(conversation_id)
025        selection_generation = next_generation
026        load_state = Loading { conversation_id, generation: next_generation }
027        clear streaming/thinking ephemera only
028        publish once
029        RETURN BeganSelection { generation: next_generation }
030    LET next_generation = current selection_generation + 1
031    selected_conversation_id = Some(conversation_id)
032    selected_title = lookup_title_or_fallback(conversation_id)
033    selection_generation = next_generation
034    load_state = Loading { conversation_id, generation: next_generation }
035    clear streaming/thinking ephemera only
036    publish once
037    RETURN BeganSelection { generation: next_generation }
038
039  FUNCTION tokio_presenter_handle_select_conversation(id, selection_generation):
040    conversation_service.set_active(id)
041    emit ConversationActivated {
042      id,
043      selection_generation,
044    }
045    load transcript asynchronously using request token selection_generation
046    IF load succeeds:
047      emit ConversationMessagesLoaded {
048        conversation_id: id,
049        selection_generation,
050        messages,
051      }
052    IF load fails:
053      emit ConversationLoadFailed {
054        conversation_id: id,
055        selection_generation,
056        message,
057      }
058
059  FUNCTION reduce ConversationActivated { id, selection_generation }:
060    IF selected_conversation_id == Some(id)
061      AND selection_generation == current selection_generation
062      AND load_state == Loading { conversation_id: id, generation: selection_generation }:
063        RETURN no-op idempotent echo
064    IF selected_conversation_id == Some(id)
065      AND selection_generation == current selection_generation:
066        maybe upgrade selected title only from `LiteralFallback("Untitled Conversation")` to `HistoryBacked(non_empty_title)` when authoritative history data now provides that stronger title
067        RETURN publish once only if that bounded provenance upgrade changed authoritative state
068    IF selection_generation < current selection_generation:
069      RETURN stale no-op
070    IF selection_generation > current selection_generation:
071      RETURN protocol-violation no-op because begin_selection(...) is the sole ordinary-runtime minting site
072    RETURN no-op
073
074  FUNCTION reduce ConversationMessagesLoaded { conversation_id, selection_generation, messages }:
075    IF selected_conversation_id != Some(conversation_id):
076      DROP as stale / off-target
077      RETURN
078    IF selection_generation != current selection_generation:
079      DROP as stale / superseded
080      RETURN
081    transcript = replace_all(messages)
082    load_state = Ready { conversation_id, generation: selection_generation }
083    publish once
084
085  FUNCTION reduce ConversationLoadFailed { conversation_id, selection_generation, message }:
086    IF selected_conversation_id != Some(conversation_id):
087      DROP
088      RETURN
089    IF selection_generation != current selection_generation:
090      DROP
091      RETURN
092    load_state = Error {
093      conversation_id,
094      generation: selection_generation,
095      message,
096    }
097    preserve previous transcript until user changes selection or retries
098    publish once
099
100  RULE: no unconditional transcript clear on ConversationActivated
101  RULE: bulk replacement remains ConversationMessagesLoaded semantics
102  RULE: publication limits apply per reducer invocation / drained batch, not per whole user selection lifecycle; one ordinary selection may legitimately publish Loading and later publish Ready/Error in separate reducer invocations
103  RULE: UI shows explicit loading/error affordance from load_state
104  RULE: selected id/title may update before transcript replacement without rendering false empty state
105  RULE: empty conversation reaches Ready only by explicit zero-message ConversationMessagesLoaded for the current generation
106  RULE: activation, success, and failure commands must all carry the same selection_generation token for one request
107  RULE: stale generation payloads are ignored without mutating transcript, load_state, or publication revision
108  RULE: popup timing is never part of freshness validation
109  RULE: tests must target observable harness behavior, not concrete future store module internals
110  RULE: same-id reselection while Loading/Ready is a strict no-op with no ephemera clear or selected-title rewrite; same-id reselection from Error retries with a new generation
```
