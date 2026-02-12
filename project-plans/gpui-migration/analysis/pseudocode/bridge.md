# Bridge Components Pseudocode

## Overview
Cross-runtime communication between GPUI (smol) and tokio via flume channels.

---

## GpuiBridge (Lines 1-60)

```
1.  // Bridge from GPUI side - emits UserEvents, receives ViewCommands
2.  STRUCT GpuiBridge {
3.    // Send UserEvents to tokio EventBus
4.    user_event_tx: Sender<UserEvent>,
5.    // Receive ViewCommands from presenters
6.    view_cmd_rx: Receiver<ViewCommand>,
7.    // Notifier to wake GPUI event loop
8.    notifier: Arc<GpuiNotifier>,
9.  }
10. 
11. IMPL GpuiBridge:
12.   FUNCTION new(
13.     user_event_tx: Sender<UserEvent>,
14.     view_cmd_rx: Receiver<ViewCommand>,
15.   ) -> Self:
16.     Self {
17.       user_event_tx,
18.       view_cmd_rx,
19.       notifier: Arc::new(GpuiNotifier::new()),
20.     }
21.   
22.   // Emit a UserEvent (called from GPUI click handlers)
23.   FUNCTION emit_user_event(&self, event: UserEvent):
24.     // Non-blocking send
25.     MATCH self.user_event_tx.try_send(event):
26.       Ok(()) => {}
27.       Err(TrySendError::Full(event)) => {
28.         tracing::warn!("UserEvent channel full, dropping: {:?}", event)
29.       }
30.       Err(TrySendError::Disconnected(_)) => {
31.         tracing::error!("UserEvent channel disconnected")
32.       }
33.   
34.   // Try to receive pending ViewCommands (non-blocking)
35.   FUNCTION try_recv_view_command(&self) -> Option<ViewCommand>:
36.     self.view_cmd_rx.try_recv().ok()
37.   
38.   // Drain all pending ViewCommands
39.   FUNCTION drain_view_commands(&self) -> Vec<ViewCommand>:
40.     LET mut commands = Vec::new()
41.     WHILE LET Some(cmd) = self.try_recv_view_command():
42.       commands.push(cmd)
43.     commands
44.   
45.   // Get notifier for waking GPUI
46.   FUNCTION notifier(&self) -> Arc<GpuiNotifier>:
47.     self.notifier.clone()
48.   
49.   // Check if wake was requested
50.   FUNCTION check_wake(&self) -> bool:
51.     self.notifier.check_and_clear()
```

## ViewCommandSink (Lines 61-100)

```
52. // Sink for presenters to send ViewCommands to GPUI
53. STRUCT ViewCommandSink {
54.   tx: Sender<ViewCommand>,
55.   notifier: Arc<GpuiNotifier>,
56. }
57. 
58. IMPL ViewCommandSink:
59.   FUNCTION new(tx: Sender<ViewCommand>, notifier: Arc<GpuiNotifier>) -> Self:
60.     Self { tx, notifier }
61.   
62.   // Send a ViewCommand to GPUI (called from presenter layer)
63.   ASYNC FUNCTION send(&self, cmd: ViewCommand) -> Result<(), BridgeError>:
64.     // Async send with backpressure
65.     self.tx.send_async(cmd).await
66.       .map_err(|_| BridgeError::ChannelClosed)?
67.     
68.     // Wake GPUI to process the command
69.     self.notifier.notify()
70.     Ok(())
71.   
72.   // Try send without blocking (for fire-and-forget)
73.   FUNCTION try_send(&self, cmd: ViewCommand) -> Result<(), BridgeError>:
74.     MATCH self.tx.try_send(cmd):
75.       Ok(()) => {
76.         self.notifier.notify()
77.         Ok(())
78.       }
79.       Err(TrySendError::Full(cmd)) => {
80.         // Log and drop on overflow
81.         tracing::warn!("ViewCommand channel full, dropping: {:?}", cmd)
82.         Err(BridgeError::ChannelFull)
83.       }
84.       Err(TrySendError::Disconnected(_)) => {
85.         Err(BridgeError::ChannelClosed)
86.       }
87. 
88. IMPL Clone FOR ViewCommandSink:
89.   FUNCTION clone(&self) -> Self:
90.     Self {
91.       tx: self.tx.clone(),
92.       notifier: self.notifier.clone(),
93.     }
```

## GpuiNotifier (Lines 101-130)

```
94. // Atomic notifier for waking GPUI event loop
95. STRUCT GpuiNotifier {
96.   wake: AtomicBool,
97. }
98. 
99. IMPL GpuiNotifier:
100.   FUNCTION new() -> Self:
101.     Self {
102.       wake: AtomicBool::new(false),
103.     }
104.   
105.   // Signal that GPUI should wake and process commands
106.   FUNCTION notify(&self):
107.     self.wake.store(true, Ordering::SeqCst)
108.   
109.   // Check and clear wake flag (returns true if was set)
110.   FUNCTION check_and_clear(&self) -> bool:
111.     self.wake.swap(false, Ordering::SeqCst)
112. 
113. // Errors
114. ENUM BridgeError {
115.   ChannelFull,
116.   ChannelClosed,
117. }
```

## TrayBridge (Lines 131-180)

```
118. // Bridge for NSStatusItem clicks to GPUI
119. STRUCT TrayBridge {
120.   // Receiver for tray click events
121.   rx: Receiver<TrayClick>,
122.   // Sender (held by Objective-C callback)
123.   tx: Sender<TrayClick>,
124.   // Reference to NSStatusItem for position queries
125.   status_item: Retained<NSStatusItem>,
126. }
127. 
128. ENUM TrayClick {
129.   Toggle,
130.   Quit,
131. }
132. 
133. IMPL TrayBridge:
134.   FUNCTION new() -> Self:
135.     LET (tx, rx) = flume::bounded::<TrayClick>(16)
136.     
137.     // Create NSStatusItem (using existing code from main_menubar.rs)
138.     LET status_bar = unsafe { NSStatusBar::systemStatusBar() }
139.     LET status_item = unsafe {
140.       status_bar.statusItemWithLength(NSStatusItemVariableLength)
141.     }
142.     
143.     // Set icon
144.     LET icon_data = include_bytes!("../../assets/MenuBarIcon.imageset/icon-32.png")
145.     LET image = load_image_data(icon_data)
146.     unsafe { status_item.button().unwrap().setImage(Some(&image)) }
147.     
148.     // Set click action (calls into tx)
149.     setup_click_action(&status_item, tx.clone())
150.     
151.     Self { rx, tx, status_item }
152.   
153.   // Get frame of status item button for positioning popup
154.   FUNCTION get_status_item_frame(&self) -> CGRect:
155.     unsafe {
156.       LET button = self.status_item.button().unwrap()
157.       LET window = button.window().unwrap()
158.       window.convertRectToScreen(button.frame())
159.     }
160. 
161. // Setup click action (Objective-C interop)
162. UNSAFE FUNCTION setup_click_action(status_item: &NSStatusItem, tx: Sender<TrayClick>):
163.   // Create action selector and target
164.   // This mirrors existing code in main_menubar.rs
165.   LET button = status_item.button().unwrap()
166.   
167.   // Store tx in static or associated object
168.   // On click: tx.send(TrayClick::Toggle)
169.   button.setAction(Some(sel!(togglePopover:)))
170.   button.setTarget(Some(/* action handler */))
```

## Channel Capacities (Lines 181-200)

```
171. // Channel configuration
172. CONST USER_EVENT_CHANNEL_CAPACITY: usize = 256
173. CONST VIEW_COMMAND_CHANNEL_CAPACITY: usize = 1024
174. 
175. // Create bridge pair
176. FUNCTION create_bridge_pair() -> (GpuiBridge, ViewCommandSink):
177.   LET (user_event_tx, user_event_rx) = flume::bounded(USER_EVENT_CHANNEL_CAPACITY)
178.   LET (view_cmd_tx, view_cmd_rx) = flume::bounded(VIEW_COMMAND_CHANNEL_CAPACITY)
179.   
180.   LET notifier = Arc::new(GpuiNotifier::new())
181.   
182.   LET gpui_bridge = GpuiBridge {
183.     user_event_tx,
184.     view_cmd_rx,
185.     notifier: notifier.clone(),
186.   }
187.   
188.   LET view_cmd_sink = ViewCommandSink {
189.     tx: view_cmd_tx,
190.     notifier,
191.   }
192.   
193.   (gpui_bridge, view_cmd_sink)
```

## Event Forwarder (Lines 201-230)

```
194. // Forwards UserEvents from flume to tokio EventBus
195. ASYNC FUNCTION forward_user_events(
196.   rx: Receiver<UserEvent>,
197.   event_bus: Arc<EventBus>,
198. ):
199.   LOOP:
200.     MATCH rx.recv_async().await:
201.       Ok(user_event) => {
202.         LET app_event = AppEvent::User(user_event)
203.         IF LET Err(e) = event_bus.publish(app_event).await:
204.           tracing::error!("Failed to publish UserEvent: {}", e)
205.       }
206.       Err(flume::RecvError::Disconnected) => {
207.         tracing::info!("UserEvent channel closed, stopping forwarder")
208.         BREAK
209.       }
```

---

## File: `src/ui_gpui/bridge/mod.rs`
## Total Line Count: ~230
## Exports: GpuiBridge, ViewCommandSink, GpuiNotifier, TrayBridge, TrayClick, create_bridge_pair
