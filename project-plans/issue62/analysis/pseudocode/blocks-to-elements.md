# Pseudocode: blocks_to_elements()

**Phase:** 02 - Pseudocode Design  
**Artifact ID:** blocks-to-elements.md  
**Plan ID:** PLAN-20260402-MARKDOWN.P02

---

## Overview

Numbered-line pseudocode for the IR-to-GPUI translator that converts `Vec<MarkdownBlock>` to `Vec<AnyElement>`. This function implements Phase 2 of the two-phase IR pipeline.

---

## Pseudocode

```
001: FUNCTION blocks_to_elements(blocks: &[MarkdownBlock]) -> Vec<AnyElement>    // REQ-MD-RENDER-040
002:     results = Vec::new()                                                     // REQ-MD-RENDER-040
003:     
004:     FOR block IN blocks DO                                                   // REQ-MD-RENDER-040
005:         element = render_block(block, 0)                                     // REQ-MD-RENDER-040
006:         APPEND results, element                                              // REQ-MD-RENDER-040
007:     END FOR
008:     
009:     RETURN results                                                           // REQ-MD-RENDER-040
010: END FUNCTION

011: // === PARAGRAPH RENDERING ================================================
012: FUNCTION render_paragraph(spans: &[MarkdownInline], links: &[(Range<usize>, String)], depth: usize) -> AnyElement
013:     // Convert spans to text runs and plain text string                     // REQ-MD-RENDER-003
014:     (runs, plain_text) = spans_to_text_runs(spans)                          // REQ-MD-RENDER-003, REQ-MD-RENDER-011
015:     
016:     IF links.is_empty() THEN                                               // REQ-MD-RENDER-042
017:         // No links: use plain StyledText                                    // REQ-MD-RENDER-042
018:         styled = StyledText::with_runs(                                     // REQ-MD-RENDER-011
019:             plain_text.into(),                                               // REQ-MD-RENDER-011
020:             runs,                                                            // REQ-MD-RENDER-011
021:             window,                                                          // REQ-MD-RENDER-011
022:             cx                                                                // REQ-MD-RENDER-011
023:         )
024:         text_element = styled.into_any_element()                            // REQ-MD-RENDER-011
025:     ELSE
026:         // Has links: use InteractiveText                                   // REQ-MD-RENDER-042
027:         click_ranges = links.iter().map(|(r, _)| r.clone()).collect()       // REQ-MD-RENDER-042
028:         urls = links.iter().map(|(_, u)| u.clone()).collect::<Vec<_>>()     // REQ-MD-RENDER-042
029:         
030:         interactive = InteractiveText::new(                                 // REQ-MD-RENDER-042
031:             ElementId::Name(format!("para-{}", element_counter.next()).into()),
032:             plain_text.into(),                                                // REQ-MD-RENDER-042
033:             runs,                                                              // REQ-MD-RENDER-042
034:             click_ranges                                                      // REQ-MD-RENDER-042
035:         )
036:         
037:         interactive = interactive.on_click(cx, |_this, idx, _window, cx| {   // REQ-MD-RENDER-042
038:             url = &urls[idx]                                                  // REQ-MD-RENDER-042
039:             IF is_safe_url(url) THEN                                          // REQ-MD-SEC-001, REQ-MD-RENDER-042
040:                 cx.open_url(url)                                              // REQ-MD-RENDER-042
041:             END IF
042:             // Silently ignore unsafe URLs - no error to user              // REQ-MD-SEC-006
043:         })
044:         
045:         text_element = interactive.into_any_element()                         // REQ-MD-RENDER-042
046:     END IF
047:     
048:     // Wrap in paragraph container with vertical spacing                     // REQ-MD-RENDER-003
049:     container = div()                                                         // REQ-MD-RENDER-003
050:         .flex()                                                               // REQ-MD-RENDER-003
051:         .flex_col()                                                           // REQ-MD-RENDER-003
052:         .w_full()                                                             // REQ-MD-RENDER-003
053:         .text_size(px(Theme::FONT_SIZE_MD))                                 // REQ-MD-RENDER-031
054:         .text_color(Theme::text_primary())                                    // REQ-MD-RENDER-031
055:         .child(text_element)                                                  // REQ-MD-RENDER-003
056:     
057:     RETURN container.into_any_element()                                      // REQ-MD-RENDER-003
058: END FUNCTION

059: // === HEADING RENDERING ==================================================
060: FUNCTION render_heading(level: u8, spans: &[MarkdownInline], links: &[(Range<usize>, String)]) -> AnyElement
061:     // Convert spans to text runs                                            // REQ-MD-RENDER-004
062:     (runs, plain_text) = spans_to_text_runs(spans)                          // REQ-MD-RENDER-004
063:     
064:     // Calculate heading size based on level                                 // REQ-MD-RENDER-033
065:     font_size = match level {                                                 // REQ-MD-RENDER-033
066:         1 => px(Theme::FONT_SIZE_LG + 8.0),  // H1: 24px                     // REQ-MD-RENDER-033
067:         2 => px(Theme::FONT_SIZE_LG + 4.0),  // H2: 20px                     // REQ-MD-RENDER-033
068:         3 => px(Theme::FONT_SIZE_LG + 2.0),  // H3: 18px                     // REQ-MD-RENDER-033
069:         4 => px(Theme::FONT_SIZE_LG),         // H4: 16px                     // REQ-MD-RENDER-033
070:         5 => px(Theme::FONT_SIZE_BASE),       // H5: 14px                     // REQ-MD-RENDER-033
071:         _ => px(Theme::FONT_SIZE_MD),        // H6: 13px                     // REQ-MD-RENDER-033
072:     }
073:     
074:     // Add bold weight to all heading runs                                   // REQ-MD-RENDER-034
075:     FOR run IN runs.mut_iter() DO                                             // REQ-MD-RENDER-034
076:         run.font.weight = FontWeight::BOLD                                  // REQ-MD-RENDER-034
077:     END FOR
078:     
079:     IF links.is_empty() THEN
080:         text_element = StyledText::with_runs(plain_text.into(), runs, window, cx).into_any_element()
081:     ELSE
082:         click_ranges = links.iter().map(|(r, _)| r.clone()).collect()        // REQ-MD-RENDER-042
083:         urls = links.iter().map(|(_, u)| u.clone()).collect::<Vec<_>>()      // REQ-MD-RENDER-042
084:         
085:         interactive = InteractiveText::new(
086:             ElementId::Name(format!("h{level}-{}", element_counter.next()).into()),
087:             plain_text.into(),
088:             runs,
089:             click_ranges
090:         ).on_click(cx, |_this, idx, _window, cx| {
091:             url = &urls[idx]
092:             IF is_safe_url(url) THEN
093:                 cx.open_url(url)
094:             END IF
095:         })
096:         
097:         text_element = interactive.into_any_element()
098:     END IF
099:     
100:     container = div()
101:         .flex()
102:         .flex_col()
103:         .w_full()
104:         .text_size(font_size)                                                  // REQ-MD-RENDER-033
105:         .font_weight(FontWeight::BOLD)                                        // REQ-MD-RENDER-034
106:         .text_color(Theme::text_primary())                                    // REQ-MD-RENDER-031
107:         .child(text_element)                                                   // REQ-MD-RENDER-004
108:     
109:     RETURN container.into_any_element()                                       // REQ-MD-RENDER-004
110: END FUNCTION

111: // === CODE BLOCK RENDERING =================================================
112: FUNCTION render_code_block(language: &Option<String>, code: &str) -> AnyElement
113:     // Create monospace font for code                                        // REQ-MD-RENDER-005, REQ-MD-RENDER-026
114:     code_font = Font::system_font().with_family("Menlo".into())               // REQ-MD-RENDER-026
115:     
116:     // Single text run for entire code block                                // REQ-MD-RENDER-005
117:     code_run = TextRun {                                                       // REQ-MD-RENDER-005
118:         len: code.len(),                                                      // REQ-MD-RENDER-005
119:         font: code_font,                                                      // REQ-MD-RENDER-005
120:         color: Theme::text_primary(),                                         // REQ-MD-RENDER-031
121:         background_color: None,                                               // REQ-MD-RENDER-005
122:         underline: None,
123:         strikethrough: None,
124:     }
125:     
126:     code_text = StyledText::with_runs(code.into(), vec![code_run], window, cx) // REQ-MD-RENDER-005
127:     
128:     // Container with code block styling                                     // REQ-MD-RENDER-005
129:     container = div()                                                          // REQ-MD-RENDER-005
130:         .flex()                                                               // REQ-MD-RENDER-005
131:         .flex_col()                                                           // REQ-MD-RENDER-005
132:         .w_full()
133:         .p(px(Theme::SPACING_MD))                                             // REQ-MD-RENDER-005
134:         .rounded(px(Theme::RADIUS_MD))                                        // REQ-MD-RENDER-005
135:         .bg(Theme::bg_darker())                                               // REQ-MD-RENDER-005
136:     
137:     // Add language label if present                                        // REQ-MD-PARSE-005
138:     IF language.is_some() THEN                                                 // REQ-MD-PARSE-005
139:         lang_label = div()                                                     // REQ-MD-PARSE-005
140:             .text_size(px(Theme::FONT_SIZE_XS))                              // REQ-MD-PARSE-005
141:             .text_color(Theme::text_muted())                                  // REQ-MD-PARSE-005
142:             .child(language.as_ref().unwrap().clone())                        // REQ-MD-PARSE-005
143:         container = container.child(lang_label)                              // REQ-MD-PARSE-005
144:     END IF
145:     
146:     container = container.child(code_text)                                   // REQ-MD-RENDER-005
147:     
148:     RETURN container.into_any_element()                                       // REQ-MD-RENDER-005
149: END FUNCTION

150: // === BLOCKQUOTE RENDERING =================================================
151: FUNCTION render_blockquote(children: &[MarkdownBlock], depth: usize) -> AnyElement
152:     // Recursively render child blocks                                       // REQ-MD-RENDER-006
153:     child_elements = Vec::new()
154:     FOR child IN children DO                                                   // REQ-MD-RENDER-006
155:         child_el = render_block(child, depth + 1)                            // REQ-MD-RENDER-006
156:         APPEND child_elements, child_el                                        // REQ-MD-RENDER-006
157:     END FOR
158:     
159:     container = div()                                                          // REQ-MD-RENDER-007
160:         .flex()                                                               // REQ-MD-RENDER-007
161:         .flex_col()                                                           // REQ-MD-RENDER-007
162:         .w_full()
163:         .pl(px(Theme::SPACING_MD))                                           // REQ-MD-RENDER-007
164:         .border_l_2()                                                         // REQ-MD-RENDER-007
165:         .border_color(Theme::accent())                                        // REQ-MD-RENDER-007
166:         .bg(Theme::bg_base())                                                 // REQ-MD-RENDER-007
167:         .p(px(Theme::SPACING_SM))                                             // REQ-MD-RENDER-007
168:         .rounded(px(Theme::RADIUS_SM))                                        // REQ-MD-RENDER-007
169:         .children(child_elements)                                             // REQ-MD-RENDER-006
170:     
171:     RETURN container.into_any_element()                                       // REQ-MD-RENDER-007
172: END FUNCTION

173: // === LIST RENDERING =======================================================
174: FUNCTION render_list(ordered: bool, start: u64, items: &[Vec<MarkdownBlock>], depth: usize) -> AnyElement
175:     item_elements = Vec::new()
176:     
177:     FOR (idx, item_blocks) IN items.iter().enumerate() DO                     // REQ-MD-RENDER-008
178:         // Calculate bullet or number prefix                                // REQ-MD-RENDER-009
179:         prefix = IF ordered THEN
180:             format!("{}.", start + idx as u64)                                // REQ-MD-RENDER-008
181:         ELSE
182:             "-"  // bullet character                                          // REQ-MD-RENDER-007
183:         END IF
184:         
185:         // Render item content blocks                                        // REQ-MD-RENDER-008
186:         item_content = Vec::new()
187:         FOR block IN item_blocks DO                                            // REQ-MD-RENDER-008
188:             block_el = render_block(block, depth + 1)                          // REQ-MD-RENDER-008
189:             APPEND item_content, block_el                                      // REQ-MD-RENDER-008
190:         END FOR
191:         
192:         // Indent based on nesting depth                                     // REQ-MD-RENDER-008
193:         indent = px(depth * 12.0)  // 12px per nesting level                 // REQ-MD-RENDER-008
194:         
195:         item_container = div()                                                 // REQ-MD-RENDER-008
196:             .flex()                                                           // REQ-MD-RENDER-008
197:             .flex_row()                                                       // REQ-MD-RENDER-008
198:             .w_full()
199:             .pl(indent)                                                       // REQ-MD-RENDER-008
200:         
201:         // Prefix column                                                     // REQ-MD-RENDER-009
202:         prefix_div = div()                                                     // REQ-MD-RENDER-009
203:             .w(px(24.0))  // Fixed width for alignment                        // REQ-MD-RENDER-009
204:             .text_color(Theme::text_muted())                                  // REQ-MD-RENDER-025
205:             .child(prefix)                                                     // REQ-MD-RENDER-009
206:         
207:         item_container = item_container.child(prefix_div)                     // REQ-MD-RENDER-009
208:         
209:         // Content column                                                    // REQ-MD-RENDER-008
210:         content_div = div()                                                    // REQ-MD-RENDER-008
211:             .flex()                                                           // REQ-MD-RENDER-008
212:             .flex_col()                                                       // REQ-MD-RENDER-008
213:             .flex_1()  // Take remaining space                               // REQ-MD-RENDER-008
214:             .children(item_content)                                           // REQ-MD-RENDER-008
215:         
216:         item_container = item_container.child(content_div)                    // REQ-MD-RENDER-008
217:         
218:         APPEND item_elements, item_container.into_any_element()              // REQ-MD-RENDER-008
219:     END FOR
220:     
221:     container = div()                                                          // REQ-MD-RENDER-008
222:         .flex()                                                               // REQ-MD-RENDER-008
223:         .flex_col()                                                           // REQ-MD-RENDER-008
224:         .gap(px(Theme::SPACING_XS))                                           // REQ-MD-RENDER-008
225:         .w_full()
226:         .children(item_elements)                                            // REQ-MD-RENDER-008
227:     
228:     RETURN container.into_any_element()                                       // REQ-MD-RENDER-008
229: END FUNCTION

230: // === TABLE RENDERING ========================================================
231: FUNCTION render_table(alignments: &[Alignment], header: &[TableCell], rows: &[Vec<TableCell>]) -> AnyElement
232:     // Calculate column count from header                                    // REQ-MD-PARSE-009
233:     col_count = header.len()                                                   // REQ-MD-PARSE-009
234:     IF col_count == 0 THEN
235:         // Empty table fallback
236:         RETURN div().child("[empty table]").into_any_element()
237:     END IF
238:     
239:     // Collect all cells for grid layout                                     // REQ-MD-RENDER-010
240:     all_cells = Vec::new()
241:     
242:     // Render header row                                                     // REQ-MD-RENDER-010
243:     FOR (col_idx, cell) IN header.iter().enumerate() DO                       // REQ-MD-RENDER-010
244:         (runs, plain_text) = spans_to_text_runs(&cell.spans)                   // REQ-MD-RENDER-011
245:         
246:         // Add bold to header runs                                           // REQ-MD-RENDER-051
247:         FOR run IN runs.mut_iter() DO                                          // REQ-MD-RENDER-051
248:             run.font.weight = FontWeight::SEMIBOLD                             // REQ-MD-RENDER-051
249:         END FOR
250:         
251:         IF cell.links.is_empty() THEN
252:             cell_text = StyledText::with_runs(plain_text.into(), runs, window, cx).into_any_element()
253:         ELSE
254:             click_ranges = cell.links.iter().map(|(r, _)| r.clone()).collect()
255:             urls = cell.links.iter().map(|(_, u)| u.clone()).collect::<Vec<_>>()
256:             
257:             interactive = InteractiveText::new(
258:                 ElementId::Name(format!("th-{}-{}", row_counter, col_idx).into()),
259:                 plain_text.into(),
260:                 runs,
261:                 click_ranges
262:             ).on_click(cx, |_this, idx, _window, cx| {
263:                 IF is_safe_url(&urls[idx]) THEN cx.open_url(&urls[idx])
264:             })
265:             
266:             cell_text = interactive.into_any_element()
267:         END IF
268:         
269:         cell_div = div()                                                        // REQ-MD-RENDER-010
270:             .px(px(Theme::SPACING_MD))                                        // REQ-MD-RENDER-010
271:             .py(px(Theme::SPACING_SM))                                        // REQ-MD-RENDER-010
272:             .bg(Theme::bg_dark())                                               // REQ-MD-RENDER-051
273:             .border_1()                                                         // REQ-MD-RENDER-053
274:             .border_color(Theme::border())                                     // REQ-MD-RENDER-053
275:             .child(cell_text)                                                  // REQ-MD-RENDER-010
276:         
277:         APPEND all_cells, cell_div                                            // REQ-MD-RENDER-010
278:     END FOR
279:     
280:     // Render body rows                                                    // REQ-MD-RENDER-010
281:     FOR (row_idx, row) IN rows.iter().enumerate() DO                          // REQ-MD-RENDER-010
282:         FOR (col_idx, cell) IN row.iter().enumerate() DO                      // REQ-MD-RENDER-010
283:             (runs, plain_text) = spans_to_text_runs(&cell.spans)               // REQ-MD-RENDER-011
284:             
285:             IF cell.links.is_empty() THEN
286:                 cell_text = StyledText::with_runs(plain_text.into(), runs, window, cx).into_any_element()
287:             ELSE
288:                 click_ranges = cell.links.iter().map(|(r, _)| r.clone()).collect()
289:                 urls = cell.links.iter().map(|(_, u)| u.clone()).collect::<Vec<_>>()
290:                 
291:                 interactive = InteractiveText::new(
292:                     ElementId::Name(format!("td-{}-{}", row_idx, col_idx).into()),
293:                     plain_text.into(),
294:                     runs,
295:                     click_ranges
296:                 ).on_click(cx, |_this, idx, _window, cx| {
297:                     IF is_safe_url(&urls[idx]) THEN cx.open_url(&urls[idx])
298:                 })
299:                 
300:                 cell_text = interactive.into_any_element()
301:             END IF
302:             
303:             // Alternating row background                                    // REQ-MD-RENDER-052
304:             bg_color = IF row_idx % 2 == 0 THEN
305:                 Theme::bg_base()  // Even rows                                // REQ-MD-RENDER-052
306:             ELSE
307:                 Theme::bg_darker()  // Odd rows                               // REQ-MD-RENDER-052
308:             END IF
309:             
310:             cell_div = div()                                                    // REQ-MD-RENDER-010
311:                 .px(px(Theme::SPACING_MD))                                    // REQ-MD-RENDER-010
312:                 .py(px(Theme::SPACING_SM))                                    // REQ-MD-RENDER-010
313:                 .bg(bg_color)                                                   // REQ-MD-RENDER-052
314:                 .border_1()                                                     // REQ-MD-RENDER-053
315:                 .border_color(Theme::border())                                 // REQ-MD-RENDER-053
316:                 .child(cell_text)                                              // REQ-MD-RENDER-010
317:             
318:             APPEND all_cells, cell_div                                        // REQ-MD-RENDER-010
319:         END FOR
320:     END FOR
321:     
322:     // Create grid container                                                // REQ-MD-RENDER-010
323:     table_container = div()                                                     // REQ-MD-RENDER-010
324:         .grid()                                                               // REQ-MD-RENDER-010
325:         .grid_cols(col_count as u16)                                          // REQ-MD-RENDER-010
326:         .gap(px(0.0))  // No gap, borders handle separation                   // REQ-MD-RENDER-053
327:         .w_full()
328:         .border_1()                                                           // REQ-MD-RENDER-053
329:         .border_color(Theme::border())                                        // REQ-MD-RENDER-053
330:         .children(all_cells)                                                  // REQ-MD-RENDER-010
331:     
332:     RETURN table_container.into_any_element()                                 // REQ-MD-RENDER-010
333: END FUNCTION

334: // === THEMATIC BREAK RENDERING ===============================================
335: FUNCTION render_thematic_break() -> AnyElement
336:     // Horizontal rule as thin colored line                                 // REQ-MD-RENDER-012
337:     rule = div()                                                               // REQ-MD-RENDER-012
338:         .h(px(1.0))                                                           // REQ-MD-RENDER-012
339:         .w_full()                                                             // REQ-MD-RENDER-012
340:         .bg(Theme::border())                                                  // REQ-MD-RENDER-012
341:         .my(px(Theme::SPACING_MD))                                            // REQ-MD-RENDER-012
342:     
343:     RETURN rule.into_any_element()                                            // REQ-MD-RENDER-012
344: END FUNCTION

345: // === IMAGE FALLBACK RENDERING ===============================================
346: FUNCTION render_image_fallback(alt: &str) -> AnyElement
347:     // Fallback text: [image: alt_text]                                       // REQ-MD-RENDER-013
348:     fallback_text = format!("[image: {}]", alt)                               // REQ-MD-RENDER-013
349:     
350:     container = div()                                                          // REQ-MD-RENDER-013
351:         .text_color(Theme::text_muted())                                     // REQ-MD-RENDER-013
352:         .text_size(px(Theme::FONT_SIZE_SM))                                  // REQ-MD-RENDER-013
353:         .child(fallback_text)                                                 // REQ-MD-RENDER-013
354:     
355:     RETURN container.into_any_element()                                       // REQ-MD-RENDER-013
356: END FUNCTION

357: // === BLOCK DISPATCH =========================================================
358: FUNCTION render_block(block: &MarkdownBlock, depth: usize) -> AnyElement
359:     MATCH block
360:         MarkdownBlock::Paragraph { spans, links } =>                         // REQ-MD-RENDER-003
361:             RETURN render_paragraph(spans, links, depth)                     // REQ-MD-RENDER-003
362:         
363:         MarkdownBlock::Heading { level, spans, links } =>                      // REQ-MD-RENDER-004
364:             RETURN render_heading(*level, spans, links)                      // REQ-MD-RENDER-004
365:         
366:         MarkdownBlock::CodeBlock { language, code } =>                        // REQ-MD-RENDER-005
367:             RETURN render_code_block(language, code)                         // REQ-MD-RENDER-005
368:         
369:         MarkdownBlock::BlockQuote { blocks } =>                                // REQ-MD-RENDER-006, REQ-MD-RENDER-007
370:             RETURN render_blockquote(blocks, depth)                          // REQ-MD-RENDER-006
371:         
372:         MarkdownBlock::List { ordered, start, items } =>                        // REQ-MD-RENDER-008
373:             RETURN render_list(*ordered, *start, items, depth)              // REQ-MD-RENDER-008
374:         
375:         MarkdownBlock::Table { alignments, header, rows } =>                    // REQ-MD-RENDER-010
376:             RETURN render_table(alignments, header, rows)                    // REQ-MD-RENDER-010
377:         
378:         MarkdownBlock::ThematicBreak =>                                         // REQ-MD-RENDER-012
379:             RETURN render_thematic_break()                                    // REQ-MD-RENDER-012
380:         
381:         MarkdownBlock::ImageFallback { alt } =>                                 // REQ-MD-RENDER-013
382:             RETURN render_image_fallback(alt)                               // REQ-MD-RENDER-013
383:     END MATCH
384: END FUNCTION

385: // === HELPER: spans_to_text_runs =============================================
386: FUNCTION spans_to_text_runs(spans: &[MarkdownInline]) -> (Vec<TextRun>, String)
387:     runs = Vec::new()                                                          // REQ-MD-RENDER-011
388:     plain_text = String::new()                                                 // REQ-MD-RENDER-011
389:     
390:     FOR span IN spans DO                                                       // REQ-MD-RENDER-011
391:         // Build font for this span                                           // REQ-MD-RENDER-011
392:         font = Font::system_font()                                             // REQ-MD-RENDER-026
393:         
394:         IF span.code THEN                                                        // REQ-MD-RENDER-023
395:             font = font.with_family("Menlo".into())                           // REQ-MD-RENDER-026
396:         END IF
397:         
398:         IF span.bold THEN                                                        // REQ-MD-RENDER-014
399:             font = font.with_weight(FontWeight::BOLD)                         // REQ-MD-RENDER-014
400:         END IF
401:         
402:         IF span.italic THEN                                                      // REQ-MD-RENDER-015
403:             font = font.with_style(FontStyle::Italic)                         // REQ-MD-RENDER-015
404:         END IF
405:         
406:         underline = IF span.link_url.is_some() THEN                             // REQ-MD-RENDER-024
407:             Some(UnderlineStyle {                                                // REQ-MD-RENDER-024
408:                 thickness: px(1.0),
409:                 color: Some(Theme::accent()),
410:                 dash_style: DashStyle::Solid,
411:             })
412:         ELSE
413:             None
414:         END IF
415:         
416:         strikethrough = IF span.strikethrough THEN                              // REQ-MD-RENDER-016
417:             Some(StrikethroughStyle {                                          // REQ-MD-RENDER-016
418:                 thickness: px(1.0),
419:                 color: Some(Theme::text_muted()),
420:             })
421:         ELSE
422:             None
423:         END IF
424:         
425:         background = IF span.code THEN                                           // REQ-MD-RENDER-023
426:             Some(Theme::bg_darker())                                           // REQ-MD-RENDER-023
427:         ELSE
428:             None
429:         END IF
430:         
431:         text_color = IF span.link_url.is_some() THEN                            // REQ-MD-RENDER-024
432:             Theme::accent()                                                    // REQ-MD-RENDER-024
433:         ELSE
434:             Theme::text_primary()                                              // REQ-MD-RENDER-031
435:         END IF
436:         
437:         text_run = TextRun {                                                   // REQ-MD-RENDER-011
438:             len: span.text.len(),  // UTF-8 bytes                              // REQ-MD-RENDER-060
439:             font: font,                                                           // REQ-MD-RENDER-011
440:             color: text_color,                                                    // REQ-MD-RENDER-031
441:             background_color: background,                                         // REQ-MD-RENDER-023
442:             underline: underline,                                                 // REQ-MD-RENDER-024
443:             strikethrough: strikethrough,                                         // REQ-MD-RENDER-016
444:         }
445:         
446:         APPEND runs, text_run                                                    // REQ-MD-RENDER-011
447:         plain_text += &span.text                                                 // REQ-MD-RENDER-011
448:     END FOR
449:     
450:     RETURN (runs, plain_text)                                                   // REQ-MD-RENDER-011
451: END FUNCTION

452: // === HELPER: is_safe_url ====================================================
453: FUNCTION is_safe_url(raw: &str) -> bool                                       // REQ-MD-SEC-001
454:     trimmed = raw.trim()                                                        // REQ-MD-SEC-001
455:     
456:     MATCH Url::parse(trimmed)                                                   // REQ-MD-SEC-001
457:         Ok(parsed) => {                                                         // REQ-MD-SEC-001
458:             scheme = parsed.scheme()                                             // REQ-MD-SEC-001
459:             RETURN scheme == "https" OR scheme == "http"                        // REQ-MD-SEC-002, REQ-MD-SEC-003
460:         }
461:         Err(_) => RETURN false                                                   // REQ-MD-SEC-001
462:     END MATCH
463: END FUNCTION                                                                  // REQ-MD-SEC-001

464: // === HELPER: has_links (recursive link detection) ==========================
465: FUNCTION has_links(blocks: &[MarkdownBlock]) -> bool                           // REQ-MD-INTEGRATE-024
466:     FOR block IN blocks DO                                                       // REQ-MD-INTEGRATE-024
467:         MATCH block
468:             MarkdownBlock::Paragraph { links, .. } =>                           // REQ-MD-INTEGRATE-024
469:                 IF links.len() > 0 THEN RETURN true                            // REQ-MD-INTEGRATE-024
470:             MarkdownBlock::Heading { links, .. } =>                              // REQ-MD-INTEGRATE-024
471:                 IF links.len() > 0 THEN RETURN true                            // REQ-MD-INTEGRATE-024
472:             
473:             MarkdownBlock::BlockQuote { blocks } =>                              // REQ-MD-INTEGRATE-024
474:                 IF has_links(blocks) THEN RETURN true                          // REQ-MD-INTEGRATE-024
475:             
476:             MarkdownBlock::List { items, .. } =>                                 // REQ-MD-INTEGRATE-024
477:                 FOR item_blocks IN items DO                                     // REQ-MD-INTEGRATE-024
478:                     IF has_links(item_blocks) THEN RETURN true                // REQ-MD-INTEGRATE-024
479:                 END FOR
480:             
481:             MarkdownBlock::Table { header, rows, .. } =>                        // REQ-MD-INTEGRATE-024
482:                 FOR cell IN header DO                                           // REQ-MD-INTEGRATE-024
483:                     IF cell.links.len() > 0 THEN RETURN true                 // REQ-MD-INTEGRATE-024
484:                 END FOR
485:                 FOR row IN rows DO                                              // REQ-MD-INTEGRATE-024
486:                     FOR cell IN row DO                                          // REQ-MD-INTEGRATE-024
487:                         IF cell.links.len() > 0 THEN RETURN true              // REQ-MD-INTEGRATE-024
488:                     END FOR
489:                 END FOR
490:             
491:             _ => {}  // Other variants have no links                           // REQ-MD-INTEGRATE-024
492:         END MATCH
493:     END FOR
494:     
495:     RETURN false                                                                 // REQ-MD-INTEGRATE-024
496: END FUNCTION
```

---

## Summary

This pseudocode covers:
- **Lines 1-10**: Function signature and main loop (REQ-MD-RENDER-040)
- **Lines 11-57**: Paragraph rendering with InteractiveText for links (REQ-MD-RENDER-003, REQ-MD-RENDER-042)
- **Lines 58-108**: Heading rendering with level-based sizing (REQ-MD-RENDER-004, REQ-MD-RENDER-033, REQ-MD-RENDER-034)
- **Lines 109-147**: Code block rendering with monospace font and language label (REQ-MD-RENDER-005)
- **Lines 148-170**: Blockquote rendering with left border and recursive children (REQ-MD-RENDER-006, REQ-MD-RENDER-007)
- **Lines 171-227**: List rendering with bullet/number prefixes and depth indentation (REQ-MD-RENDER-008, REQ-MD-RENDER-009)
- **Lines 228-331**: Table rendering with grid layout, header cells, alternating rows (REQ-MD-RENDER-010, REQ-MD-RENDER-051-053)
- **Lines 332-342**: ThematicBreak rendering (REQ-MD-RENDER-012)
- **Lines 343-354**: ImageFallback rendering (REQ-MD-RENDER-013)
- **Lines 355-382**: Block dispatch function
- **Lines 383-449**: spans_to_text_runs helper - converts MarkdownInline to TextRun + plain text (REQ-MD-RENDER-011)
- **Lines 450-461**: is_safe_url helper - URL validation for security (REQ-MD-SEC-001 through REQ-MD-SEC-003)
- **Lines 462-495**: has_links helper - recursive link detection for click-to-copy behavior (REQ-MD-INTEGRATE-024)

All pseudocode lines reference requirement IDs and theme method calls.
