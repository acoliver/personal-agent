# GitHub Issue #93: Approval UX follow-up: queue prompts, show tool arguments, and group related requests

**Repository:** personal-agent
**State:** open

## Body

Problem
When the agent triggers multiple approval-required tool calls in a short burst, the approval prompts visually stack over each other. This makes it look like the approve button is not working, even when the user did approve the previous request.

Observed pain points
- Approval prompts appear on top of each other instead of being clearly sequenced.
- Prompt content often does not include enough actionable context (for example file path for edit/write, directory path for search/glob, full command for shell).
- Multiple related operations (especially repeated edits for the same file) are shown as separate approvals even when they are part of one logical action.

Requested behavior
1) Queue approvals instead of overlap
- Approval requests should be shown one at a time in a deterministic queue.
- After approve or deny, the next request should appear clearly.

2) Show meaningful argument context for every approval
- Approval UI should display a compact, tool-specific summary of arguments.
- Examples:
  - Edit/Write tools: target file path (and relevant line scope where applicable)
  - Search/Glob/List tools: target directory/path and pattern
  - Shell tool: exact command text
  - MCP tools: server name, tool name, and key argument summary

3) Group related approvals when safe and understandable
- If multiple edit-like operations target the same file in the same run step, present as one grouped approval with expandable details.
- Grouping should preserve clear user control and auditability.

Scope requirements
- This should be designed as a cross-tool approval UX improvement, not an EditFile-only patch.
- Must apply consistently to built-in tools and MCP-provided tools.
- Needs a clear policy for when requests are grouped vs kept separate.

Acceptance criteria
- No visual overlap of approval prompts during rapid multi-tool runs.
- Each prompt includes useful argument context by default.
- Related same-target operations can be grouped in one approval surface.
- Behavior is consistent across built-in and MCP tools.
- Tests cover queue ordering, argument rendering, and grouping rules.

