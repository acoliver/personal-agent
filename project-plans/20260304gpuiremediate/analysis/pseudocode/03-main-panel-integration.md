# Pseudocode 03: MainPanel and Popup Integration After Recovery

## Overview

`MainPanel` becomes a thin renderer/composition root over authoritative snapshots.

## Integration Flow (Lines 1-144)

```text
001  FUNCTION main_gpui_startup():
002    construct authoritative app store before popup open
003    construct bridge and presenter tasks
004    launch spawn_runtime_bridge_pump(...) from the Application::run GPUI app root using cx.spawn(...)
005    expose store handle globally in GPUI runtime state
006    expose handle_select_conversation_intent(...) globally in GPUI runtime state
007    collect startup hydration commands/snapshots needed for first frame
008    run reduce_startup_batch(startup_inputs) to completion before popup subscription/mount when startup already knows selected transcript data
009    commit the final startup snapshot/revision
010    no popup subscriber exists yet by contract
011    any publication at this point is a silent no-op by contract
012    only then allow MainPanel subscription/mount to read current_snapshot()
013
014  FUNCTION spawn_runtime_bridge_pump(app_store, bridge, cx):
015    LET in_flight_guard = runtime_owned_single_flight_guard()
016    cx.spawn(async move |_cx| loop {
017      WAIT on one fixed GPUI background-executor timer tick (16ms cadence unless preflight explicitly approves equivalent bounded cadence)
018      IF in_flight_guard indicates active critical section:
019        SKIP this tick
020        CONTINUE
021      ENTER in_flight_guard critical section
022      LET commands = bridge.drain_commands()
023      IF commands.is_empty():
024        EXIT in_flight_guard critical section
025        CONTINUE
026      app_store.reduce_batch(commands)
027      EXIT in_flight_guard critical section
028    })
029    EXIT only when runtime-owned bridge/store context is shutting down or no longer valid
030    RULE: this is the sole production callsite for bridge.drain_commands() after Phase 05
031    RULE: it remains alive with zero mounted popups/windows
032    RULE: it must not depend on MainPanel, popup_window, or child-view lifetime
033    RULE: one pump iteration drains one bridge batch by calling drain_commands() exactly once, invokes one reducer entrypoint, and may cause at most one publication for that batch
034    RULE: do not add a second inner drain-until-empty loop around drain_commands(); commands arriving after the drain begins are handled by a later tick
035    RULE: the single-flight guard covers the whole drain_commands() -> reducer execution -> revision bump -> publication completion window
036
037  FUNCTION handle_select_conversation_intent(conversation_id):
038    LET result = app_store.begin_selection(conversation_id)
039    MATCH result:
040      NoOpSameSelection =>
041        RETURN
042      BeganSelection { generation } =>
043        LET sent = gpui_bridge.emit(UserEvent::SelectConversation {
044          id: conversation_id,
045          selection_generation: generation,
046        })
047        IF sent == false:
048          app_store.reduce_batch([
049            ConversationLoadFailed {
050              conversation_id,
051              selection_generation: generation,
052              message: "SelectConversation transport enqueue failed",
053            }
054          ])
055        RETURN
056
057  FUNCTION open_popup():
058    mount MainPanel
059    MainPanel subscribes to store snapshots
060    MainPanel reads current_snapshot() immediately on subscription
061    MainPanel renders latest snapshot immediately
062    popup does not need special startup_commands to be correct
063
064  STRUCT MainPanel {
065    navigation
066    store_subscription
067    latest_snapshot
068    child_views
069    optional_transport_glue
070  }
071
072  FUNCTION MainPanel::init():
073    subscribe to store snapshot updates
074    read current_snapshot() immediately
075    create child views with initial snapshot slices
076    request redraw on snapshot revision change
077    DO NOT own authoritative transcript state
078
079  FUNCTION MainPanel::poll_bridge_during_migration():
080    optionally proxy/schedule spawn_runtime_bridge_pump(...)
081    DO NOT become the sole drainer after Phase 05
082    DO NOT forward transcript/state commands directly into child views as the authority path once Phase 05 lands
083
084  FUNCTION MainPanel::handle_store_snapshot(snapshot):
085    latest_snapshot = snapshot
086    forward snapshot slices to mounted children
087    notify window redraw
088
089  FUNCTION MainPanel::handle_intent(intent):
090    route selection intents to handle_select_conversation_intent(...)
091    route other raw user intents toward existing user-event/presenter path
092    DO NOT synthesize startup-only transcript replay
093
094  FUNCTION ChatView::render(snapshot.chat):
095    derive selected title, transcript, loading state, streaming state from snapshot
096    emit intents only
097
098  FUNCTION HistoryView::render(snapshot.history):
099    derive list and selected conversation from snapshot
100    emit selection intents only
101
102  MIGRATION STAGES:
103    Stage 1 / Phase 04:
104      store exists, is process-lifetime, and supports subscriptions
105      runtime-owned bridge pump seam is introduced from main_gpui root state, but runtime command semantics may still arrive through old delivery plumbing during this transitional phase
106      popup-local forwarding/bootstrap paths remain explicitly transitional rather than target authority
107
108    Stage 2 / Phase 05:
109      production-path bridge-drained runtime commands reduce into store first through spawn_runtime_bridge_pump(...)
110      handle_select_conversation_intent(...) + begin_selection(conversation_id) own ordinary-runtime freshness issuance
111      MainPanel transport glue may remain temporarily, but direct command forwarding is no longer the semantic state owner and may not advance authoritative generation
112      snapshot revisions become the authority for selected conversation/loading state
113
114    Stage 3 / Phase 06:
115      startup hydration uses reduce_startup_batch(startup_inputs) as the sole startup semantic mutator in the same reducer module and same publication discipline
116      startup batch publishes only final visible state for already-known transcript data
117      startup_commands may exist only as compatibility glue that populates store first and never publishes an intermediate incorrect frame
118
119    Stage 4 / Phase 08:
120      no remaining startup/bootstrap semantic authority lives in MainPanel
121      remaining MainPanel responsibilities are navigation, popup lifecycle, intent routing, snapshot subscription/redraw, and any tightly bounded transport-only glue
122
123  RULE: popup closed => store continues receiving runtime updates through spawn_runtime_bridge_pump(...)
124  RULE: popup reopened => MainPanel gets latest snapshot without replay coupling
125  RULE: bridge polling may remain transport detail during migration but not authority boundary
126  RULE: startup_commands become temporary migration shim only if they populate store first and never publish an intermediate incorrect frame
127  RULE: final deprecation removes redundant bootstrap application once snapshot hydration proves equivalent
128
129  VERIFICATION TARGETS:
130    selected id/title and transcript count stay coherent on manual switch
131    startup first frame remains correct
132    startup hydration does not flash empty/loading before known transcript appears
133    all startup reductions complete before first subscriber-visible startup state
134    one mandatory combined startup proof exists: subscriber-visible snapshot/revision observation plus first-subscriber current_snapshot() readback show that the startup batch committed before popup mount, any pre-subscription publication was silent by contract, and the first subscriber/render reads the already-committed startup snapshot
135    popup reopen preserves latest transcript snapshot
136    zero-subscriber runtime mutation still reaches store and becomes visible on later reopen
137    ConversationMessagesLoaded still performs bulk replacement
138    ConversationActivated does not clear transcript by itself
139    MainPanel responsibilities shrink measurably
140    semantic tests prove store, not popup timing, owns correctness
141    selection_generation freshness is visible in the reducer path
142    anti-mirror behavior is proven by behavior evidence, not only by comments or grep results
143    proof artifacts name the dropped popup-local objects and the surviving store handle identity across unmount/remount
144    any surviving runtime helper is proven transport-only and incapable of holding independent selected transcript/loading/title/generation authority
```
