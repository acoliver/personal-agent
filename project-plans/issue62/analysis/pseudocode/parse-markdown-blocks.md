# Pseudocode: parse_markdown_blocks()

**Phase:** 02 - Pseudocode Design  
**Artifact ID:** parse-markdown-blocks.md  
**Plan ID:** PLAN-20260402-MARKDOWN.P02

---

## Overview

Numbered-line pseudocode for the pulldown-cmark event walker that produces `Vec<MarkdownBlock>`. This function implements Phase 1 of the two-phase IR pipeline.

---

## Pseudocode

```
001: FUNCTION parse_markdown_blocks(content: &str) -> Vec<MarkdownBlock>          // REQ-MD-PARSE-001
002:     // Setup pulldown-cmark options for Phase A parsing
003:     options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS  // REQ-MD-PARSE-011
004:     
005:     // Create parser with content and options
006:     parser = Parser::new_ext(content, options)                                // REQ-MD-PARSE-001
007:     
008:     // Initialize accumulators for IR construction
009:     blocks = Vec::new()                      // Final accumulated blocks
010:     block_stack = Vec::new()                 // BlockBuilder stack for nesting
011:     inline_stack = Vec::new()                  // Inline style state stack
012:     list_stack = Vec::new()                    // ListContext stack for bullets/numbers
013:     html_buffer = String::new()                // Accumulator for HTML block content
014:     in_html_block = false                    // HTML block state flag
015:     text_buffer = String::new()              // Current inline text accumulator
016:     current_spans = Vec::new()                 // MarkdownInline spans for current block
017:     current_links = Vec::new()                 // (Range<usize>, String) tuples for links
018:     link_start_offset = 0                    // Byte offset when link starts
019:     current_url = None                       // URL when inside a link
020:     
021:     // Main event loop - process all pulldown-cmark events
022:     FOR event IN parser DO                                                    // REQ-MD-PARSE-001
023:         
024:         // === BLOCK-LEVEL EVENTS: Paragraph ====================================
025:         IF event == Start(Paragraph) THEN                                     // REQ-MD-PARSE-002
026:             PUSH block_stack, BlockBuilder::Paragraph { spans: Vec::new(), links: Vec::new() }
027:             text_buffer = ""                                                  // REQ-MD-PARSE-002
028:             current_spans = Vec::new()                                          // REQ-MD-PARSE-002
029:             current_links = Vec::new()                                          // REQ-MD-PARSE-002
030:         
031:         ELSE IF event == End(Paragraph) THEN                                  // REQ-MD-PARSE-002
032:             IF NOT block_stack.is_empty() THEN
033:                 builder = POP block_stack                                     // REQ-MD-PARSE-002
034:                 // Flush any remaining text to a span
035:                 IF text_buffer.len() > 0 THEN
036:                     span = create_inline_span(text_buffer, &inline_stack)     // REQ-MD-PARSE-030
037:                     APPEND current_spans, span                                  // REQ-MD-PARSE-030
038:                 END IF
039:                 // TRANSFER: Move current_spans and current_links to builder
040:                 builder.spans = current_spans                                     // REQ-MD-PARSE-002
041:                 builder.links = current_links                                     // REQ-MD-PARSE-002
042:                 current_spans = Vec::new()                                          // REQ-MD-PARSE-002
043:                 current_links = Vec::new()                                          // REQ-MD-PARSE-002
044:                 block = MarkdownBlock::Paragraph { spans: builder.spans, links: builder.links }
045:                 APPEND blocks, block                                            // REQ-MD-PARSE-002
046:                 text_buffer = ""                                                // REQ-MD-PARSE-002
047:             END IF
048:         
049:         // === BLOCK-LEVEL EVENTS: Heading ======================================
050:         ELSE IF event == Start(Heading { level, .. }) THEN                    // REQ-MD-PARSE-003
051:             PUSH block_stack, BlockBuilder::Heading { level, spans: Vec::new(), links: Vec::new() }
052:             text_buffer = ""                                                  // REQ-MD-PARSE-003
053:             current_spans = Vec::new()                                          // REQ-MD-PARSE-003
054:             current_links = Vec::new()                                          // REQ-MD-PARSE-003
055:         
056:         ELSE IF event == End(Heading) THEN                                    // REQ-MD-PARSE-003
057:             IF NOT block_stack.is_empty() THEN
058:                 builder = POP block_stack                                     // REQ-MD-PARSE-003
059:                 IF text_buffer.len() > 0 THEN
060:                     span = create_inline_span(text_buffer, &inline_stack)
061:                     APPEND current_spans, span
062:                 END IF
063:                 // TRANSFER: Move current_spans and current_links to builder
064:                 builder.spans = current_spans
065:                 builder.links = current_links
066:                 current_spans = Vec::new()
067:                 current_links = Vec::new()
068:                 block = MarkdownBlock::Heading { level: builder.level, spans: builder.spans, links: builder.links }
069:                 APPEND blocks, block                                            // REQ-MD-PARSE-003
070:                 text_buffer = ""
071:             END IF
072:         
073:         // === BLOCK-LEVEL EVENTS: CodeBlock ===================================
074:         ELSE IF event == Start(CodeBlock(info)) THEN                            // REQ-MD-PARSE-004
075:             language = extract_language(info)                                 // REQ-MD-PARSE-005
076:             PUSH block_stack, BlockBuilder::CodeBlock { language, code: "" }   // REQ-MD-PARSE-004
077:             text_buffer = ""                                                    // REQ-MD-PARSE-004
078:         
079:         ELSE IF event == End(CodeBlock) THEN                                  // REQ-MD-PARSE-004
080:             IF NOT block_stack.is_empty() THEN
081:                 builder = POP block_stack                                     // REQ-MD-PARSE-004
082:                 block = MarkdownBlock::CodeBlock { language: builder.language, code: text_buffer }
083:                 APPEND blocks, block                                            // REQ-MD-PARSE-004
084:                 text_buffer = ""
085:             END IF
086:         
087:         // === BLOCK-LEVEL EVENTS: BlockQuote ===================================
088:         ELSE IF event == Start(BlockQuote(_)) THEN                            // REQ-MD-PARSE-006
089:             // Push marker for blockquote context
090:             PUSH block_stack, BlockBuilder::BlockQuote { children: Vec::new() }  // REQ-MD-PARSE-006
091:             // Blockquote children go into separate nested blocks list
092:             
093:         ELSE IF event == End(BlockQuote) THEN                                 // REQ-MD-PARSE-006
094:             IF NOT block_stack.is_empty() THEN
095:                 builder = POP block_stack                                     // REQ-MD-PARSE-006
096:                 block = MarkdownBlock::BlockQuote { children: builder.children }
097:                 APPEND blocks, block                                            // REQ-MD-PARSE-006
098:             END IF
099:         
100:         // === BLOCK-LEVEL EVENTS: List =========================================
101:         ELSE IF event == Start(List(Some(n))) THEN                            // REQ-MD-PARSE-008
102:             PUSH list_stack, ListContext::Ordered { next_number: n }          // REQ-MD-PARSE-008
103:             PUSH block_stack, BlockBuilder::List { ordered: true, start: n, items: Vec::new(), current_item: Vec::new() }
104:         
105:         ELSE IF event == Start(List(None)) THEN                               // REQ-MD-PARSE-007
106:             PUSH list_stack, ListContext::Unordered                             // REQ-MD-PARSE-007
107:             PUSH block_stack, BlockBuilder::List { ordered: false, start: 0, items: Vec::new(), current_item: Vec::new() }
108:         
109:         ELSE IF event == End(List) THEN                                       // REQ-MD-PARSE-007, REQ-MD-PARSE-008
110:             IF NOT block_stack.is_empty() AND NOT list_stack.is_empty() THEN
111:                 builder = POP block_stack                                     // REQ-MD-PARSE-007
112:                 POP list_stack                                                  // REQ-MD-PARSE-007
113:                 // Flush last item if exists
114:                 IF builder.current_item.len() > 0 THEN
115:                     APPEND builder.items, builder.current_item                    // REQ-MD-PARSE-007
116:                 END IF
117:                 block = MarkdownBlock::List { ordered: builder.ordered, start: builder.start, items: builder.items }
118:                 APPEND blocks, block                                            // REQ-MD-PARSE-007
119:             END IF
120:         
121:         ELSE IF event == Start(Item) THEN                                     // REQ-MD-PARSE-007
122:             // Start collecting blocks for this list item
123:             IF NOT block_stack.is_empty() THEN
124:                 builder = PEEK block_stack
125:                 IF builder.current_item.len() > 0 THEN
126:                     APPEND builder.items, builder.current_item                    // REQ-MD-PARSE-007
127:                     builder.current_item = Vec::new()                             // REQ-MD-PARSE-007
128:                 END IF
129:                 // Push item marker for nested block detection
130:                 PUSH block_stack, BlockBuilder::ItemMarker
131:             END IF
132:         
133:         ELSE IF event == End(Item) THEN                                       // REQ-MD-PARSE-007
134:             // Pop until ItemMarker, collecting blocks into item
135:             item_blocks = Vec::new()
136:             WHILE NOT block_stack.is_empty() AND TOP(block_stack) != ItemMarker DO
137:                 block = POP block_stack
138:                 PREPEND item_blocks, block                                      // Maintain order
139:             END WHILE
140:             POP block_stack  // Remove ItemMarker
141:             IF NOT block_stack.is_empty() THEN
142:                 list_builder = PEEK block_stack
143:                 APPEND list_builder.current_item, item_blocks                   // REQ-MD-PARSE-007
144:             END IF
145:         
146:         // === BLOCK-LEVEL EVENTS: Table ========================================
147:         ELSE IF event == Start(Table(alignments)) THEN                        // REQ-MD-PARSE-009
148:             aln = alignments.iter().map(Into::into).collect::<Vec<Alignment>>() // REQ-MD-PARSE-009
149:             PUSH block_stack, BlockBuilder::Table { alignments: aln, header: Vec::new(), rows: Vec::new(), current_row: Vec::new(), in_header: false }
150:             
151:         ELSE IF event == Start(TableHead) THEN                                 // REQ-MD-PARSE-009
152:             IF NOT block_stack.is_empty() THEN
153:                 builder = PEEK block_stack
154:                 builder.in_header = true                                        // REQ-MD-PARSE-009
155:             END IF
156:         
157:         ELSE IF event == End(TableHead) THEN                                  // REQ-MD-PARSE-009
158:             IF NOT block_stack.is_empty() THEN
159:                 builder = PEEK block_stack
160:                 builder.in_header = false                                       // REQ-MD-PARSE-009
161:             END IF
162:         
163:         ELSE IF event == Start(TableRow) THEN                                 // REQ-MD-PARSE-009
164:             IF NOT block_stack.is_empty() THEN
165:                 builder = PEEK block_stack
166:                 builder.current_row = Vec::new()                                // REQ-MD-PARSE-009
167:             END IF
168:         
169:         ELSE IF event == End(TableRow) THEN                                   // REQ-MD-PARSE-009
170:             IF NOT block_stack.is_empty() THEN
171:                 builder = PEEK block_stack
172:                 IF builder.in_header THEN
173:                     APPEND builder.header, builder.current_row                    // REQ-MD-PARSE-009
174:                 ELSE
175:                     APPEND builder.rows, builder.current_row                      // REQ-MD-PARSE-009
176:                 END IF
177:             END IF
178:         
179:         ELSE IF event == Start(TableCell) THEN                                // REQ-MD-PARSE-009
180:             IF NOT block_stack.is_empty() THEN
181:                 builder = PEEK block_stack
182:                 // Push cell builder onto inline context
183:                 PUSH block_stack, BlockBuilder::TableCell { spans: Vec::new(), links: Vec::new() }
184:                 text_buffer = ""
185:                 current_spans = Vec::new()
186:                 current_links = Vec::new()
187:             END IF
188:         
189:         ELSE IF event == End(TableCell) THEN                                  // REQ-MD-PARSE-009
190:             IF NOT block_stack.is_empty() THEN
191:                 cell_builder = POP block_stack
192:                 IF text_buffer.len() > 0 THEN
193:                     span = create_inline_span(text_buffer, &inline_stack)
194:                     APPEND current_spans, span
195:                 END IF
196:                 // TRANSFER: Move current_spans and current_links to cell builder
197:                 cell_builder.spans = current_spans
198:                 cell_builder.links = current_links
199:                 current_spans = Vec::new()
200:                 current_links = Vec::new()
201:                 cell = TableCell { spans: cell_builder.spans, links: cell_builder.links }
202:                 IF NOT block_stack.is_empty() THEN
203:                     table_builder = PEEK block_stack
204:                     APPEND table_builder.current_row, cell                        // REQ-MD-PARSE-009
205:                 END IF
206:                 text_buffer = ""
207:             END IF
208:         
209:         ELSE IF event == End(Table) THEN                                      // REQ-MD-PARSE-009
210:             IF NOT block_stack.is_empty() THEN
211:                 builder = POP block_stack
212:                 block = MarkdownBlock::Table { alignments: builder.alignments, header: builder.header, rows: builder.rows }
213:                 APPEND blocks, block                                            // REQ-MD-PARSE-009
214:             END IF
215:         
216:         // === BLOCK-LEVEL EVENTS: ThematicBreak ================================
217:         ELSE IF event == Rule THEN                                            // REQ-MD-PARSE-010
218:             APPEND blocks, MarkdownBlock::ThematicBreak                         // REQ-MD-PARSE-010
219:         
220:         // === INLINE STYLE EVENTS ================================================
221:         ELSE IF event == Start(Strong) THEN                                   // REQ-MD-PARSE-025
222:             // Flush current text before style change
223:             IF text_buffer.len() > 0 THEN
224:                 span = create_inline_span(text_buffer, &inline_stack)
225:                 APPEND current_spans, span
226:                 text_buffer = ""
227:             END IF
228:             PUSH inline_stack, InlineStyle::Bold                                // REQ-MD-PARSE-025
229:         
230:         ELSE IF event == End(Strong) THEN                                     // REQ-MD-PARSE-025
231:             IF text_buffer.len() > 0 THEN
232:                 span = create_inline_span(text_buffer, &inline_stack)
233:                 APPEND current_spans, span
234:                 text_buffer = ""
235:             END IF
236:             POP inline_stack                                                    // REQ-MD-PARSE-025
237:         
238:         ELSE IF event == Start(Emphasis) THEN                                 // REQ-MD-PARSE-026
239:             IF text_buffer.len() > 0 THEN
240:                 span = create_inline_span(text_buffer, &inline_stack)
241:                 APPEND current_spans, span
242:                 text_buffer = ""
243:             END IF
244:             PUSH inline_stack, InlineStyle::Italic                              // REQ-MD-PARSE-026
245:         
246:         ELSE IF event == End(Emphasis) THEN                                   // REQ-MD-PARSE-026
247:             IF text_buffer.len() > 0 THEN
248:                 span = create_inline_span(text_buffer, &inline_stack)
249:                 APPEND current_spans, span
250:                 text_buffer = ""
251:             END IF
252:             POP inline_stack                                                    // REQ-MD-PARSE-026
253:         
254:         ELSE IF event == Start(Strikethrough) THEN                              // REQ-MD-PARSE-027
255:             IF text_buffer.len() > 0 THEN
256:                 span = create_inline_span(text_buffer, &inline_stack)
257:                 APPEND current_spans, span
258:                 text_buffer = ""
259:             END IF
260:             PUSH inline_stack, InlineStyle::Strikethrough                       // REQ-MD-PARSE-027
261:         
262:         ELSE IF event == End(Strikethrough) THEN                              // REQ-MD-PARSE-027
263:             IF text_buffer.len() > 0 THEN
264:                 span = create_inline_span(text_buffer, &inline_stack)
265:                 APPEND current_spans, span
266:                 text_buffer = ""
267:             END IF
268:             POP inline_stack                                                    // REQ-MD-PARSE-027
269:         
270:         ELSE IF event == Start(Link { dest_url, .. }) THEN                    // REQ-MD-PARSE-028
271:             IF text_buffer.len() > 0 THEN
272:                 span = create_inline_span(text_buffer, &inline_stack)
273:                 APPEND current_spans, span
274:                 text_buffer = ""
275:             END IF
276:             link_start_offset = count_bytes_in_spans(&current_spans)           // REQ-MD-PARSE-028
277:             current_url = Some(dest_url.to_string())                           // REQ-MD-PARSE-028
278:             PUSH inline_stack, InlineStyle::Link(dest_url.to_string())         // REQ-MD-PARSE-028
279:         
280:         ELSE IF event == End(Link) THEN                                       // REQ-MD-PARSE-028
281:             IF text_buffer.len() > 0 THEN
282:                 span = create_inline_span(text_buffer, &inline_stack)
283:                 APPEND current_spans, span
284:                 text_buffer = ""
285:             END IF
286:             IF current_url.is_some() THEN
287:                 link_end_offset = count_bytes_in_spans(&current_spans)          // REQ-MD-PARSE-028
288:                 url = current_url.take().unwrap()
289:                 APPEND current_links, (link_start_offset..link_end_offset, url) // REQ-MD-PARSE-028
290:             END IF
291:             IF NOT inline_stack.is_empty() THEN
292:                 POP inline_stack                                                // REQ-MD-PARSE-028
293:             END IF
294:         
295:         // === TEXT EVENTS =======================================================
296:         ELSE IF event == Text(text) THEN                                      // REQ-MD-PARSE-020
297:             text_buffer += text.as_ref()                                        // REQ-MD-PARSE-020
298:         
299:         ELSE IF event == Code(text) THEN                                      // REQ-MD-PARSE-021
300:             // Inline code - emit immediately as special span
301:             code_span = MarkdownInline {
302:                 text: text.to_string(),
303:                 bold: false,
304:                 italic: false,
305:                 strikethrough: false,
306:                 code: true,                                                       // REQ-MD-PARSE-021
307:                 link_url: None,
308:             }
309:             APPEND current_spans, code_span                                      // REQ-MD-PARSE-021
310:         
311:         ELSE IF event == SoftBreak THEN                                         // REQ-MD-PARSE-022
312:             text_buffer += " "                                                   // REQ-MD-PARSE-022
313:         
314:         ELSE IF event == HardBreak THEN                                         // REQ-MD-PARSE-022
315:             text_buffer += "\n"                                                  // REQ-MD-PARSE-022
316:         
317:         // === HTML HANDLING ===================================================
318:         ELSE IF event == Start(HtmlBlock) THEN                                  // REQ-MD-PARSE-043, REQ-MD-PARSE-044
319:             in_html_block = true                                                 // REQ-MD-PARSE-043
320:             html_buffer = ""                                                      // REQ-MD-PARSE-043
321:             text_buffer = ""  // Suspend normal text accumulation
322:         
323:         ELSE IF event == End(HtmlBlock) THEN                                    // REQ-MD-PARSE-043
324:             in_html_block = false                                                // REQ-MD-PARSE-043
325:             stripped = strip_html_tags(&html_buffer)                             // REQ-MD-PARSE-043
326:             IF stripped.len() > 0 THEN
327:                 text_buffer += stripped                                          // REQ-MD-PARSE-043
328:             END IF
329:             html_buffer = ""
330:         
331:         ELSE IF event == Html(html) THEN                                        // REQ-MD-PARSE-043
332:             IF in_html_block THEN
333:                 html_buffer += html.as_ref()                                     // REQ-MD-PARSE-043
334:             ELSE
335:                 // Block-level HTML not in HtmlBlock - strip and append as paragraph
336:                 stripped = strip_html_tags(html.as_ref())                          // REQ-MD-PARSE-043
337:                 IF stripped.len() > 0 THEN
338:                     APPEND blocks, MarkdownBlock::Paragraph {                      // REQ-MD-PARSE-043
339:                         spans: vec![MarkdownInline::plain(stripped)],
340:                         links: vec![]
341:                     }
342:                 END IF
343:             END IF
344:         
345:         ELSE IF event == InlineHtml(html) THEN                                  // REQ-MD-PARSE-044
346:             // Strip inline HTML tags, append text to current buffer
347:             stripped = strip_html_tags(html.as_ref())                            // REQ-MD-PARSE-044
348:             text_buffer += stripped                                              // REQ-MD-PARSE-044
349:         
350:         // === SPECIAL ELEMENTS ==================================================
351:         ELSE IF event == Start(Image { dest_url: _, .. }) THEN                  // REQ-MD-PARSE-040
352:             // Collect alt text until End(Image)
353:             image_alt_buffer = ""                                               // REQ-MD-PARSE-040
354:             // Image handling: accumulate alt text, ignore URL
355:             
356:         // Image End handled via state tracking (simplified - actual impl needs state machine)
357:         
358:         ELSE IF event == TaskListMarker(checked) THEN                         // REQ-MD-PARSE-024
359:             // Prepend ballot box character to current span
360:             IF checked THEN
361:                 text_buffer += ""  // U+2611 checked ballot box                // REQ-MD-PARSE-024
362:             ELSE
363:                 text_buffer += ""  // U+2610 unchecked ballot box              // REQ-MD-PARSE-024
364:             END IF
365:         
366:         ELSE IF event == FootnoteReference(label) THEN                          // REQ-MD-PARSE-042
367:             text_buffer += "[^" + label + "]"                                    // REQ-MD-PARSE-042
368:         
369:         ELSE IF event == InlineMath(text) THEN                                  // REQ-MD-PARSE-049
370:             // Render math as code-styled text (monospace + bg)
371:             math_span = MarkdownInline {
372:                 text: text.to_string(),
373:                 bold: false,
374:                 italic: false,
375:                 strikethrough: false,
376:                 code: true,                                                       // REQ-MD-PARSE-049
377:                 link_url: None,
378:             }
379:             APPEND current_spans, math_span                                       // REQ-MD-PARSE-049
380:         
381:         ELSE IF event == Start(DisplayMath) THEN                                 // REQ-MD-PARSE-049
382:             // Start display math block - treat as code block
383:             PUSH block_stack, BlockBuilder::CodeBlock { language: Some("math".to_string()), code: "" }
384:             text_buffer = ""                                                      // REQ-MD-PARSE-049
385:             current_spans = Vec::new()                                              // REQ-MD-PARSE-049
386:             current_links = Vec::new()                                              // REQ-MD-PARSE-049
387:         
388:         ELSE IF event == End(DisplayMath) THEN                                   // REQ-MD-PARSE-049
389:             IF NOT block_stack.is_empty() THEN
390:                 builder = POP block_stack                                         // REQ-MD-PARSE-049
391:                 // TRANSFER: flush any remaining text
392:                 IF text_buffer.len() > 0 THEN
393:                     code_span = MarkdownInline {
394:                         text: text_buffer.clone(),
395:                         code: true,
396:                         bold: false,
397:                         italic: false,
398:                         strikethrough: false,
399:                         link_url: None,
400:                     }
401:                     APPEND current_spans, code_span
402:                 END IF
403:                 builder.spans = current_spans
404:                 current_spans = Vec::new()
405:                 // Build display math as code block using builder.spans
406:                 block = MarkdownBlock::CodeBlock { 
407:                     language: builder.language, 
408:                     code: builder.spans.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("")
409:                 }
410:                 APPEND blocks, block                                            // REQ-MD-PARSE-049
411:                 text_buffer = ""
412:             END IF
413:         
414:         ELSE IF event == Start(DefinitionList) THEN                              // REQ-MD-PARSE-049
415:             // Push definition list builder onto stack
416:             PUSH block_stack, BlockBuilder::DefinitionList { items: Vec::new(), current_term: Vec::new(), current_def: Vec::new() }
417:             current_spans = Vec::new()
418:             current_links = Vec::new()
419:         
420:         ELSE IF event == Start(DefinitionListTerm) THEN                          // REQ-MD-PARSE-049
421:             // Flush any pending definition content
422:             IF NOT block_stack.is_empty() THEN
423:                 builder = PEEK block_stack
424:                 IF builder.current_def.len() > 0 THEN
425:                     // Package current term+def as a paragraph block
426:                     term_text = current_spans.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("")
427:                     term_block = MarkdownBlock::Paragraph {
428:                         spans: vec![MarkdownInline { text: term_text, bold: true, italic: false, strikethrough: false, code: false, link_url: None }],
429:                         links: current_links.clone()
430:                     }
431:                     APPEND builder.current_def, term_block
432:                     builder.current_def = Vec::new()
433:                 END IF
434:             END IF
435:             current_spans = Vec::new()
436:             current_links = Vec::new()
437:         
438:         ELSE IF event == End(DefinitionListTerm) THEN                            // REQ-MD-PARSE-049
439:             IF NOT block_stack.is_empty() THEN
440:                 builder = PEEK block_stack
441:                 term_text = current_spans.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("")
442:                 term_block = MarkdownBlock::Paragraph {
443:                     spans: vec![MarkdownInline { text: term_text, bold: true, italic: false, strikethrough: false, code: false, link_url: None }],
444:                     links: current_links.clone()
445:                 }
446:                 APPEND builder.current_def, term_block
447:             END IF
448:             current_spans = Vec::new()
449:             current_links = Vec::new()
450:         
451:         ELSE IF event == Start(DefinitionListDefinition) THEN                    // REQ-MD-PARSE-049
452:             current_spans = Vec::new()
453:             current_links = Vec::new()
454:         
455:         ELSE IF event == End(DefinitionListDefinition) THEN                        // REQ-MD-PARSE-049
456:             IF NOT block_stack.is_empty() THEN
457:                 builder = PEEK block_stack
458:                 def_text = current_spans.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("")
459:                 def_block = MarkdownBlock::Paragraph {
460:                     spans: vec![MarkdownInline { text: "  " + &def_text, bold: false, italic: false, strikethrough: false, code: false, link_url: None }],
461:                     links: current_links.clone()
462:                 }
463:                 APPEND builder.current_def, def_block
464:             END IF
465:             current_spans = Vec::new()
466:             current_links = Vec::new()
467:         
468:         ELSE IF event == End(DefinitionList) THEN                                // REQ-MD-PARSE-049
469:             IF NOT block_stack.is_empty() THEN
470:                 builder = POP block_stack
471:                 // TRANSFER: Move current_def items into items list if any pending
472:                 IF builder.current_def.len() > 0 THEN
473:                     APPEND builder.items, builder.current_def
474:                     builder.current_def = Vec::new()
475:                 END IF
476:                 // Flatten definition list to paragraph blocks (bold terms, indented defs)
477:                 FOR item_blocks IN builder.items DO
478:                     FOR block IN item_blocks DO
479:                         APPEND blocks, block                                    // REQ-MD-PARSE-049
480:                     END FOR
481:                 END FOR
482:             END IF
483:         
484:         // === FALLBACK EVENTS ===================================================
485:         ELSE IF event == Start(Superscript) OR event == End(Superscript) THEN   // REQ-MD-PARSE-048
486:             // No-op: render as plain text                                        // REQ-MD-PARSE-048
487:             
488:         ELSE IF event == Start(Subscript) OR event == End(Subscript) THEN       // REQ-MD-PARSE-048
489:             // No-op: render as plain text                                        // REQ-MD-PARSE-048
490:         
491:         ELSE IF event == Start(MetadataBlock(_)) OR event == End(MetadataBlock(_)) THEN  // REQ-MD-PARSE-041
492:             // Skip metadata blocks entirely                                       // REQ-MD-PARSE-041
493:             
494:         ELSE IF event == Start(FootnoteDefinition(_)) OR event == End(FootnoteDefinition(_)) THEN  // REQ-MD-PARSE-042
495:             // Render footnote as plain paragraph with label prefix               // REQ-MD-PARSE-042
496:             // Unknown event: attempt to extract text content as fallback
497:             // This prevents data loss from unhandled events
498:             log::warn!("Unhandled markdown event: {:?}", event)
499:         END IF
500:         
501:     END FOR
502:     
503:     // Flush any remaining blocks from stack (handles unclosed blocks)
504:     WHILE NOT block_stack.is_empty() DO
505:         builder = POP block_stack
506:         block = finalize_builder(builder, text_buffer, current_spans, current_links)
507:         IF block.is_some() THEN
508:             APPEND blocks, block
509:         END IF
510:     END WHILE
511:     
512:     RETURN blocks                                                          // REQ-MD-PARSE-001
513: END FUNCTION

514: // === HELPER FUNCTIONS ===

515: FUNCTION create_inline_span(text: String, stack: &Vec<InlineStyle>) -> MarkdownInline
516:     bold = stack.contains(InlineStyle::Bold)                                // REQ-MD-PARSE-025
517:     italic = stack.contains(InlineStyle::Italic)                            // REQ-MD-PARSE-026
518:     strikethrough = stack.contains(InlineStyle::Strikethrough)              // REQ-MD-PARSE-027
519:     link_url = stack.iter().find_map(|s| match s { InlineStyle::Link(url) => Some(url), _ => None })  // REQ-MD-PARSE-028
520:     RETURN MarkdownInline { text, bold, italic, strikethrough, code: false, link_url }  // REQ-MD-PARSE-030
521: END FUNCTION

522: FUNCTION count_bytes_in_spans(spans: &Vec<MarkdownInline>) -> usize
523:     total = 0
524:     FOR span IN spans DO
525:         total += span.text.len()  // UTF-8 byte count                        // REQ-MD-PARSE-060
526:     END FOR
527:     RETURN total
528: END FUNCTION

529: FUNCTION strip_html_tags(html: &str) -> String
530:     // State machine: track if inside <...> and if inside script/style
531:     in_tag = false                                                         // REQ-MD-PARSE-043
532:     in_strip_tag = false                                                   // REQ-MD-PARSE-045
533:     strip_tag_name = ""                                                    // REQ-MD-PARSE-045
534:     result = String::new()                                                   // REQ-MD-PARSE-043
535:     i = 0
536:     WHILE i < html.len() DO
537:         IF html[i] == '<' AND NOT in_tag THEN                                 // REQ-MD-PARSE-043
538:             in_tag = true                                                     // REQ-MD-PARSE-043
539:             // Check if this is a script or style tag
540:             remaining = &html[i+1..]
541:             IF remaining.starts_with("script") OR remaining.starts_with("style") THEN
542:                 in_strip_tag = true                                           // REQ-MD-PARSE-045
543:                 strip_tag_name = if remaining.starts_with("script") { "script" } else { "style" }
544:             ELSE IF remaining.starts_with("/script") OR remaining.starts_with("/style") THEN
545:                 in_strip_tag = false                                          // REQ-MD-PARSE-045
546:                 strip_tag_name = ""
547:             END IF
548:             i += 1
549:         ELSE IF html[i] == '>' AND in_tag THEN                                // REQ-MD-PARSE-043
550:             in_tag = false                                                    // REQ-MD-PARSE-043
551:             i += 1
552:         ELSE IF in_tag THEN                                                   // REQ-MD-PARSE-043
553:             // Inside tag: skip character                                      // REQ-MD-PARSE-043
554:             i += 1
555:         ELSE IF in_strip_tag THEN                                             // REQ-MD-PARSE-045
556:             // Inside script/style content: strip entirely
557:             i += 1
558:         ELSE
559:             // Outside tag: append character                                   // REQ-MD-PARSE-043
560:             result.push(html[i])                                              // REQ-MD-PARSE-043
561:             i += 1
562:         END IF
563:     END WHILE
564:     // Handle malformed: if still in_tag, treat remaining as literal           // REQ-MD-PARSE-050
565:     IF in_tag THEN
566:         // Unclosed tag - append the < that started it                        // REQ-MD-PARSE-050
567:         result = "<" + result
568:     END IF
569:     RETURN result                                                            // REQ-MD-PARSE-043
570: END FUNCTION

571: FUNCTION extract_language(info: &str) -> Option<String>
572:     // Extract first word from info string as language
573:     words = info.split_whitespace().collect::<Vec<_>>()                    // REQ-MD-PARSE-005
574:     IF words.len() > 0 THEN
575:         RETURN Some(words[0].to_string())                                   // REQ-MD-PARSE-005
576:     ELSE
577:         RETURN None                                                           // REQ-MD-PARSE-005
578:     END IF
579: END FUNCTION
```

---

## Summary

This pseudocode covers:
- **Lines 1-20**: Setup and initialization (REQ-MD-PARSE-001, REQ-MD-PARSE-011)
- **Lines 21-218**: All block-level events (Paragraph, Heading, CodeBlock, BlockQuote, List, Table, ThematicBreak)
- **Lines 219-293**: Inline style events (Strong, Emphasis, Strikethrough, Link) with TRANSFER steps
- **Lines 294-315**: Text events (Text, Code, SoftBreak, HardBreak)
- **Lines 316-349**: HTML handling (HtmlBlock, Html, InlineHtml with tag stripping including script/style content stripping per REQ-MD-PARSE-045)
- **Lines 350-483**: Special elements with substantive pseudocode for DisplayMath (lines 381-412) and DefinitionList (lines 414-482)
- **Lines 484-499**: Fallback events (Superscript, Subscript, Metadata, etc.)
- **Lines 500-512**: Finalization and return with span/link transfer steps
- **Lines 513-579**: Helper functions (create_inline_span, strip_html_tags with content stripping, etc.)

All pseudocode lines reference requirement IDs from the specification.
