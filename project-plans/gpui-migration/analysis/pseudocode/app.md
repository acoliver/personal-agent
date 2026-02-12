# GPUI Application Setup Pseudocode

## Overview
Application initialization, tray bridge, and popup window management.

---

## Main Entry Point (Lines 1-50)

```
1.  // Main entry point
2.  FUNCTION main():
3.    // Initialize tokio runtime for services
4.    INIT tokio_runtime = tokio::runtime::Runtime::new()
5.    
6.    // Initialize EventBus and presenters on tokio runtime
7.    tokio_runtime.spawn_blocking(|| {
8.      INIT event_bus = EventBus::global()
9.      INIT chat_presenter = ChatPresenter::new(event_bus.clone())
10.     INIT history_presenter = HistoryPresenter::new(event_bus.clone())
11.     INIT settings_presenter = SettingsPresenter::new(event_bus.clone())
12.   })
13.   
14.   // Create GPUI bridge (flume channels for cross-runtime communication)
15.   LET (user_event_tx, user_event_rx) = flume::bounded::<UserEvent>(256)
16.   LET (view_cmd_tx, view_cmd_rx) = flume::bounded::<ViewCommand>(1024)
17.   
18.   // Create bridge components
19.   LET gpui_bridge = GpuiBridge::new(user_event_tx.clone(), view_cmd_rx.clone())
20.   LET view_cmd_sink = ViewCommandSink::new(view_cmd_tx.clone())
21.   
22.   // Wire presenters to ViewCommandSink
23.   chat_presenter.set_view_sink(view_cmd_sink.clone())
24.   history_presenter.set_view_sink(view_cmd_sink.clone())
25.   settings_presenter.set_view_sink(view_cmd_sink.clone())
26.   
27.   // Spawn event forwarder (tokio side)
28.   tokio_runtime.spawn(async move {
29.     forward_user_events(user_event_rx, event_bus).await
30.   })
31.   
32.   // Initialize GPUI Application (runs on main thread)
33.   Application::new().run(|cx| {
34.     // Create tray/status item bridge
35.     LET tray_bridge = TrayBridge::new()
36.     
37.     // Spawn tray click handler
38.     cx.spawn(|cx| async move {
39.       poll_tray_clicks(tray_bridge.rx, cx).await
40.     }).detach()
41.     
42.     // Spawn ViewCommand receiver
43.     cx.spawn(|cx| async move {
44.       poll_view_commands(view_cmd_rx, cx).await
45.     }).detach()
46.     
47.     // Initialize main panel (invisible until tray click)
48.     cx.set_global(AppState::new(gpui_bridge))
49.   })
```

## Event Forwarder (Lines 51-70)

```
51. // Forwards UserEvents from GPUI to tokio EventBus
52. ASYNC FUNCTION forward_user_events(rx: Receiver<UserEvent>, event_bus: EventBus):
53.   LOOP:
54.     MATCH rx.recv_async().await:
55.       Ok(user_event) => {
56.         LET app_event = AppEvent::User(user_event)
57.         event_bus.publish(app_event).await
58.       }
59.       Err(_) => BREAK  // Channel closed
60. 
61. // Polls tray clicks and toggles popup
62. ASYNC FUNCTION poll_tray_clicks(rx: Receiver<TrayClick>, cx: AsyncAppContext):
63.   LOOP:
64.     MATCH rx.recv_async().await:
65.       Ok(TrayClick::Toggle) => {
66.         cx.update(|cx| toggle_popup(cx))
67.       }
68.       Ok(TrayClick::Quit) => {
69.         cx.update(|cx| cx.quit())
70.       }
71.       Err(_) => BREAK
```

## Popup Window Management (Lines 72-120)

```
72. // Toggle popup visibility
73. FUNCTION toggle_popup(cx: &mut AppContext):
74.   LET app_state = cx.global::<AppState>()
75.   
76.   IF app_state.popup_window.is_some():
77.     close_popup(cx)
78.   ELSE:
79.     open_popup(cx)
80. 
81. // Open popup below status item
82. FUNCTION open_popup(cx: &mut AppContext):
83.   LET tray_bridge = cx.global::<TrayBridge>()
84.   LET status_frame = tray_bridge.get_status_item_frame()
85.   
86.   // Calculate position below status item, centered
87.   LET popup_width = 420.0
88.   LET popup_height = 600.0
89.   LET popup_x = status_frame.origin.x + (status_frame.size.width / 2.0) - (popup_width / 2.0)
90.   LET popup_y = status_frame.origin.y + status_frame.size.height + 4.0  // 4px gap
91.   
92.   LET window_options = WindowOptions {
93.     kind: WindowKind::PopUp,
94.     bounds: WindowBounds::Windowed(Bounds {
95.       origin: point(px(popup_x), px(popup_y)),
96.       size: size(px(popup_width), px(popup_height)),
97.     }),
98.     focus: true,
99.     show: true,
100.    titlebar: None,  // No title bar for popup
101.    background_appearance: WindowBackgroundAppearance::Blurred,
102.    app_id: None,
103.  }
104.  
105.  LET window = cx.open_window(window_options, |window, cx| {
106.    LET gpui_bridge = cx.global::<AppState>().bridge.clone()
107.    LET main_panel = MainPanel::new(gpui_bridge)
108.    cx.new(|_| main_panel)
109.  })
110.  
111.  // Store window handle
112.  cx.update_global::<AppState>(|state, _| {
113.    state.popup_window = Some(window)
114.  })
115. 
116. // Close popup
117. FUNCTION close_popup(cx: &mut AppContext):
118.  IF LET Some(window) = cx.global::<AppState>().popup_window:
119.    window.remove(cx)
120.  cx.update_global::<AppState>(|state, _| state.popup_window = None)
```

## ViewCommand Handler (Lines 121-150)

```
121. // Polls ViewCommands from presenters and applies to GPUI state
122. ASYNC FUNCTION poll_view_commands(rx: Receiver<ViewCommand>, cx: AsyncAppContext):
123.   LOOP:
124.     MATCH rx.recv_async().await:
125.       Ok(cmd) => {
126.         cx.update(|cx| {
127.           // Find main panel and dispatch command
128.           IF LET Some(window) = cx.global::<AppState>().popup_window:
129.             window.update(cx, |window, cx| {
130.               LET panel = window.root::<MainPanel>(cx)
131.               panel.handle_view_command(cmd, cx)
132.             })
133.         })
134.       }
135.       Err(_) => BREAK
136. 
137. // Notifier for waking GPUI event loop
138. STRUCT GpuiNotifier {
139.   wake: Arc<AtomicBool>,
140. }
141. 
142. IMPL GpuiNotifier:
143.   FUNCTION notify(&self):
144.     self.wake.store(true, Ordering::SeqCst)
145.   
146.   FUNCTION check_and_clear(&self) -> bool:
147.     self.wake.swap(false, Ordering::SeqCst)
```

---

## File: `src/ui_gpui/app.rs`
## Line Count: ~150
## Dependencies: gpui, flume, tokio
