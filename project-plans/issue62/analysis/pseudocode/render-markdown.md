# Pseudocode: render_markdown()

**Phase:** 02 - Pseudocode Design  
**Artifact ID:** render-markdown.md  
**Plan ID:** PLAN-20260402-MARKDOWN.P02

---

## Overview

Numbered-line pseudocode for the public API and integration helpers for markdown rendering. This file covers the top-level public API and security/utility helpers.

---

## Pseudocode

```
001: // === PUBLIC API: render_markdown =========================================
002: /// Render a markdown string into a vector of GPUI elements.
003: ///
004: /// This is the main public API for markdown rendering. It composes the
005: /// two-phase pipeline:
006: /// 1. parse_markdown_blocks() - converts markdown text to IR
007: /// 2. blocks_to_elements() - converts IR to GPUI elements
008: ///
009: /// # Arguments
010: /// * `content` - The markdown text to render
011: ///
012: /// # Returns
013: pub FUNCTION render_markdown(content: &str) -> Vec<AnyElement>                // REQ-MD-RENDER-040
014:     // Phase 1: Parse markdown into intermediate representation            // REQ-MD-PARSE-001
015:     blocks = parse_markdown_blocks(content)                                // REQ-MD-PARSE-001, REQ-MD-RENDER-040
016:     
017:     // Phase 2: Convert IR blocks to GPUI elements                         // REQ-MD-RENDER-040
018:     elements = blocks_to_elements(&blocks)                                  // REQ-MD-RENDER-040
019:     
020:     RETURN elements                                                         // REQ-MD-RENDER-040
021: END FUNCTION

022: // === PUBLIC API: render_markdown_with_meta =================================
023: /// Render markdown with additional metadata about the content.
024: ///
025: /// Returns both the rendered elements and metadata useful for
026: /// integration decisions (like whether to enable click-to-copy).
027: ///
028: /// # Arguments
029: /// * `content` - The markdown text to render
030: ///
031: /// # Returns
032: /// (Vec<AnyElement>, MarkdownMeta) - Elements and metadata
033: pub FUNCTION render_markdown_with_meta(content: &str) -> (Vec<AnyElement>, MarkdownMeta)  // REQ-MD-INTEGRATE-024
034:     blocks = parse_markdown_blocks(content)                                // REQ-MD-PARSE-001
035:     
036:     // Check for links to determine click-to-copy behavior                // REQ-MD-INTEGRATE-024
037:     has_link_content = has_links(&blocks)                                   // REQ-MD-INTEGRATE-024
038:     
039:     elements = blocks_to_elements(&blocks)                                  // REQ-MD-RENDER-040
040:     
041:     meta = MarkdownMeta {                                                    // REQ-MD-INTEGRATE-024
042:         has_links: has_link_content,                                        // REQ-MD-INTEGRATE-024
043:         block_count: blocks.len(),                                           // REQ-MD-INTEGRATE-024
044:     }
045:     
046:     RETURN (elements, meta)                                                   // REQ-MD-INTEGRATE-024
047: END FUNCTION

048: // === SECURITY: is_safe_url =================================================
049: /// Validate that a URL is safe to open.
050: ///
051: /// Only allows http:// and https:// schemes. All other schemes
052: /// are rejected to prevent security vulnerabilities.
053: ///
054: /// # Arguments
055: /// * `raw` - The raw URL string to validate
056: ///
057: /// # Returns
058: /// true if the URL has http or https scheme, false otherwise
059: pub FUNCTION is_safe_url(raw: &str) -> bool                                  // REQ-MD-SEC-001
060:     trimmed = raw.trim()                                                      // REQ-MD-SEC-001
061:     
062:     // Parse URL using url crate for RFC 3986 compliance                    // REQ-MD-SEC-001
063:     MATCH Url::parse(trimmed)                                                   // REQ-MD-SEC-001
064:         Ok(parsed) => {                                                         // REQ-MD-SEC-001
065:             scheme = parsed.scheme()                                            // REQ-MD-SEC-001
066:             
067:             // Check scheme against allowlist                                  // REQ-MD-SEC-002, REQ-MD-SEC-003
068:             IF scheme == "https" OR scheme == "http" THEN                        // REQ-MD-SEC-002, REQ-MD-SEC-003
069:                 RETURN true                                                     // REQ-MD-SEC-002
070:             ELSE
071:                 // Reject: javascript:, file:, data:, etc.                   // REQ-MD-SEC-002, REQ-MD-SEC-003
072:                 log::debug!("Blocked unsafe URL scheme: {}", scheme)            // REQ-MD-SEC-006
073:                 RETURN false                                                    // REQ-MD-SEC-003
074:             END IF
075:         }
076:         Err(e) => {                                                             // REQ-MD-SEC-001
077:             // Malformed URL - treat as unsafe                               // REQ-MD-SEC-001
078:             log::debug!("URL parse failed: {:?}", e)                            // REQ-MD-SEC-006
079:             RETURN false                                                        // REQ-MD-SEC-001
080:         }
081:     END MATCH
082: END FUNCTION                                                                  // REQ-MD-SEC-001

083: // === UTILITY: has_links =====================================================
084: /// Recursively check if any block contains clickable links.
085: ///
086: /// Used by AssistantBubble to determine click-to-copy behavior:
087: /// - If has_links returns false: enable bubble-level click-to-copy
088: /// - If has_links returns true: disable bubble-level click (links use InteractiveText)
089: ///
090: /// # Arguments
091: /// * `blocks` - The markdown blocks to check
092: ///
093: /// # Returns
094: /// true if any block contains links, false otherwise
095: pub FUNCTION has_links(blocks: &[MarkdownBlock]) -> bool                     // REQ-MD-INTEGRATE-024
096:     FOR block IN blocks DO                                                      // REQ-MD-INTEGRATE-024
097:         MATCH block                                                             // REQ-MD-INTEGRATE-024
098:             MarkdownBlock::Paragraph { links, .. } =>                          // REQ-MD-INTEGRATE-024
099:                 IF links.len() > 0 THEN                                         // REQ-MD-INTEGRATE-024
100:                     RETURN true                                                 // REQ-MD-INTEGRATE-024
101:                 END IF
102:             
103:             MarkdownBlock::Heading { links, .. } =>                             // REQ-MD-INTEGRATE-024
104:                 IF links.len() > 0 THEN                                         // REQ-MD-INTEGRATE-024
105:                     RETURN true                                                 // REQ-MD-INTEGRATE-024
106:                 END IF
107:             
108:             MarkdownBlock::BlockQuote { blocks: nested } =>                    // REQ-MD-INTEGRATE-024
109:                 IF has_links(nested) THEN                                        // REQ-MD-INTEGRATE-024
110:                     RETURN true                                                 // REQ-MD-INTEGRATE-024
111:                 END IF
112:             
113:             MarkdownBlock::List { items, .. } =>                                // REQ-MD-INTEGRATE-024
114:                 FOR item_blocks IN items DO                                      // REQ-MD-INTEGRATE-024
115:                     IF has_links(item_blocks) THEN                              // REQ-MD-INTEGRATE-024
116:                         RETURN true                                             // REQ-MD-INTEGRATE-024
117:                     END IF
118:                 END FOR
119:             
120:             MarkdownBlock::Table { header, rows, .. } =>                         // REQ-MD-INTEGRATE-024
121:                 // Check header cells for links                               // REQ-MD-INTEGRATE-024
122:                 FOR cell IN header DO                                            // REQ-MD-INTEGRATE-024
123:                     IF cell.links.len() > 0 THEN                                  // REQ-MD-INTEGRATE-024
124:                         RETURN true                                             // REQ-MD-INTEGRATE-024
125:                     END IF
126:                 END FOR
127:                 // Check body cells for links                                 // REQ-MD-INTEGRATE-024
128:                 FOR row IN rows DO                                               // REQ-MD-INTEGRATE-024
129:                     FOR cell IN row DO                                           // REQ-MD-INTEGRATE-024
130:                         IF cell.links.len() > 0 THEN                            // REQ-MD-INTEGRATE-024
131:                             RETURN true                                         // REQ-MD-INTEGRATE-024
132:                         END IF
133:                     END FOR
134:                 END FOR
135:             
136:            _ => {}  // Other variants have no links                           // REQ-MD-INTEGRATE-024
137:         END MATCH
138:     END FOR
139:     
140:     RETURN false                                                                // REQ-MD-INTEGRATE-024
141: END FUNCTION                                                                  // REQ-MD-INTEGRATE-024

142: // === INTEGRATION: AssistantBubble delegation pattern ========================
143: /// Pseudocode showing how AssistantBubble uses the markdown pipeline.
144: ///
145: /// This is the integration pattern for rendering assistant messages.
146: /// Shows the conditional click-to-copy logic based on link presence.

147: // In AssistantBubble::into_element():
148: PSEUDOCODE_FOR_AssistantBubble_into_element:                                  // REQ-MD-INTEGRATE-001, REQ-MD-INTEGRATE-015
149:     // Prepare content with streaming cursor if needed                       // REQ-MD-INTEGRATE-008
150:     content_text = IF self.is_streaming THEN                                  // REQ-MD-INTEGRATE-008
151:         format!("{}▋", self.content)                                          // REQ-MD-INTEGRATE-008
152:     ELSE
153:         self.content.clone()                                                    // REQ-MD-INTEGRATE-008
154:     END IF
155:     
156:     // Parse markdown to IR blocks                                             // REQ-MD-INTEGRATE-001
157:     blocks = parse_markdown_blocks(&content_text)                             // REQ-MD-INTEGRATE-001
158:     
159:     // Check for links to determine click behavior                          // REQ-MD-INTEGRATE-015, REQ-MD-INTEGRATE-016
160:     has_link_content = has_links(&blocks)                                       // REQ-MD-INTEGRATE-024
161:     
162:     // Convert IR to GPUI elements                                             // REQ-MD-INTEGRATE-001
163:     elements = blocks_to_elements(&blocks)                                      // REQ-MD-INTEGRATE-001
164:     
165:     // Build the container with normalized styling                            // REQ-MD-INTEGRATE-012
166:     container = div()                                                           // REQ-MD-INTEGRATE-001
167:         .max_w(px(300.0))                                                      // REQ-MD-INTEGRATE-012
168:         .px(px(Theme::SPACING_MD))                                            // REQ-MD-INTEGRATE-012
169:         .py(px(Theme::SPACING_SM))                                              // REQ-MD-INTEGRATE-012
170:         .rounded(px(Theme::RADIUS_LG))                                           // REQ-MD-INTEGRATE-012
171:         .bg(Theme::bg_darker())                                                  // REQ-MD-INTEGRATE-012
172:         .border_1()                                                             // REQ-MD-INTEGRATE-012
173:         .border_color(Theme::border())                                          // REQ-MD-INTEGRATE-012
174:         .text_color(Theme::text_primary())                                      // REQ-MD-INTEGRATE-001
175:         .children(elements)                                                     // REQ-MD-INTEGRATE-001
176:     
177:     // Conditionally attach click-to-copy handler                           // REQ-MD-INTEGRATE-015, REQ-MD-INTEGRATE-016
178:     IF NOT has_link_content AND NOT self.is_streaming THEN                      // REQ-MD-INTEGRATE-015
179:         // No links and not streaming: enable bubble click-to-copy          // REQ-MD-INTEGRATE-015
180:         container = container                                                   // REQ-MD-INTEGRATE-015
181:             .cursor_pointer()                                                   // REQ-MD-INTEGRATE-015
182:             .on_click({                                                          // REQ-MD-INTEGRATE-015
183:                 let content = self.content.clone()                               // REQ-MD-INTEGRATE-015
184:                 move |_event, _window, cx| {                                      // REQ-MD-INTEGRATE-015
185:                     cx.write_to_clipboard(gpui::ClipboardItem::new_string(content))  // REQ-MD-INTEGRATE-015
186:                 }
187:             })
188:     ELSE IF has_link_content THEN                                               // REQ-MD-INTEGRATE-016
189:         // Has links: InteractiveText handles clicks, bubble copy disabled    // REQ-MD-INTEGRATE-016
190:         // Cursor remains default (no cursor_pointer)                        // REQ-MD-INTEGRATE-016
191:         // Link clicks handled by InteractiveText::on_click in elements      // REQ-MD-INTEGRATE-016
192:     ELSE
193:         // Streaming: no click handler (cursor interaction reserved)        // REQ-MD-INTEGRATE-008
194:     END IF
195:     
196:     RETURN container                                                            // REQ-MD-INTEGRATE-001
197: END PSEUDOCODE

198: // === INTEGRATION: render_assistant_message delegation =======================
199: /// Pseudocode showing how render_assistant_message delegates to AssistantBubble.
200: ///
201: /// This unifies the two rendering paths (completed vs streaming).
202: 
203: PSEUDOCODE_FOR_render_assistant_message:                                      // REQ-MD-INTEGRATE-002
204:     // Get model ID with fallback                                             // REQ-MD-INTEGRATE-002
205:     model_id = msg.model_id.clone().unwrap_or_else(|| "Assistant".to_string())  // REQ-MD-INTEGRATE-002
206:     
207:     // Build AssistantBubble with message content                             // REQ-MD-INTEGRATE-002
208:     bubble = AssistantBubble::new(msg.content.clone())                          // REQ-MD-INTEGRATE-002
209:         .model_id(model_id)                                                     // REQ-MD-INTEGRATE-002
210:         .show_thinking(show_thinking)                                           // REQ-MD-INTEGRATE-002
211:         .streaming(false)                                                       // REQ-MD-INTEGRATE-002
212:     
213:     // Add thinking content if present                                        // REQ-MD-INTEGRATE-002
214:     IF msg.thinking.is_some() THEN                                              // REQ-MD-INTEGRATE-002
215:         bubble = bubble.thinking(msg.thinking.as_ref().unwrap().clone())        // REQ-MD-INTEGRATE-002
216:     END IF
217:     
218:     // Delegate all rendering to AssistantBubble                              // REQ-MD-INTEGRATE-002
219:     RETURN bubble.into_any_element()                                            // REQ-MD-INTEGRATE-002
220: END PSEUDOCODE

221: // === MODULE EXPORTS =========================================================
222: // In src/ui_gpui/components/mod.rs:
223: pub mod markdown_content;                                                     // REQ-MD-INTEGRATE-004
224: pub use markdown_content::render_markdown;                                    // REQ-MD-INTEGRATE-004
225: pub use markdown_content::render_markdown_with_meta;                          // REQ-MD-INTEGRATE-024
```

---

## Summary

This pseudocode covers:

- **Lines 1-21**: `render_markdown()` public API - composes parse + render phases (REQ-MD-RENDER-040, REQ-MD-PARSE-001)
- **Lines 22-47**: `render_markdown_with_meta()` public API - returns elements + metadata (REQ-MD-INTEGRATE-024)
- **Lines 48-82**: `is_safe_url()` security helper - URL validation with scheme allowlist (REQ-MD-SEC-001 through REQ-MD-SEC-006)
- **Lines 83-141**: `has_links()` utility - recursive link detection across all block types (REQ-MD-INTEGRATE-024)
- **Lines 142-197**: AssistantBubble integration pseudocode - shows conditional click-to-copy based on link presence (REQ-MD-INTEGRATE-001, REQ-MD-INTEGRATE-015, REQ-MD-INTEGRATE-016)
- **Lines 198-220**: render_assistant_message integration pseudocode - delegation to AssistantBubble (REQ-MD-INTEGRATE-002)
- **Lines 221-225**: Module export declarations (REQ-MD-INTEGRATE-004)

---

## Key Design Decisions

1. **Two-Phase Pipeline**: The public API `render_markdown()` simply composes `parse_markdown_blocks()` and `blocks_to_elements()`. This keeps the API simple while enabling internal flexibility.

2. **Metadata Variant**: `render_markdown_with_meta()` returns metadata about the content (specifically `has_links`) that AssistantBubble needs for click-to-copy decisions without parsing twice.

3. **Security-First URLs**: `is_safe_url()` uses a positive allowlist (http/https only) rather than a denylist. Failed parses are silently rejected.

4. **Link Detection**: `has_links()` recursively walks all block variants. This is necessary because links can appear in nested contexts (blockquotes, lists, tables).

5. **Integration Patterns**: The pseudocode shows exactly how AssistantBubble and render_assistant_message use the pipeline, including the conditional click handler attachment logic.
