# MCP UI Flow - Wireframes

This document contains all wireframes for the MCP user interface.
For full specification, see [SPEC.md](./SPEC.md).

---

## Settings Panel (Entry Point)

```
+------------------------------------------+
| < Settings                  Refresh Models|
+------------------------------------------+
| Profiles                                  |
| +--------------------------------------+ |
| | > Claude Sonnet 4               [*]  | |
| | > GPT-4o                              | |
| +--------------------------------------+ |
| |  -   +   Edit                        | |
| +--------------------------------------+ |
|                                          |
| MCPs                                     |
| +--------------------------------------+ |
| | [x] GitHub                           | |
| | [ ] Filesystem                       | |
| +--------------------------------------+ |
| |  -   +   Edit                        | |
| +--------------------------------------+ |
|                                          |
| Global Hotkey: [Cmd+Shift+Space    ]    |
+------------------------------------------+
```

- Checkbox = enabled/disabled
- `-` = delete selected
- `+` = add new
- `Edit` = edit selected

---

## Add MCP Screen (Initial State)

```
+------------------------------------------+
| < Add MCP                                 |
+------------------------------------------+
|                                          |
| URL: [________________________________]  |
|                                          |
| -- or search registry --                 |
|                                          |
| Registry: [Select...              v]     |
|            - Official                    |
|            - Smithery                    |
|            - Both                        |
|                                          |
| Search:   [________________________]     |
|           (select registry first)        |
|                                          |
| +--------------------------------------+ |
| |                                      | |
| |                                      | |
| |                                      | |
| +--------------------------------------+ |
|                                          |
|                                  [Next]  |
+------------------------------------------+
```

- URL: direct entry (npx command, docker image, HTTP URL)
- Registry: must select before search is enabled
- Search: disabled until registry selected
- Next: enabled when URL has content OR result selected

---

## Add MCP Screen (No Results)

```
+------------------------------------------+
| < Add MCP                                 |
+------------------------------------------+
|                                          |
| URL: [________________________________]  |
|                                          |
| -- or search registry --                 |
|                                          |
| Registry: [Official               v]     |
| Search:   [xyznonexistent_________]      |
|                                          |
| +--------------------------------------+ |
| |                                      | |
| |   No MCPs found matching             | |
| |   "xyznonexistent"                   | |
| |                                      | |
| |   Try a different search term or     | |
| |   paste a URL directly.              | |
| |                                      | |
| +--------------------------------------+ |
|                                          |
|                                  [Next]  |
+------------------------------------------+
```

---

## Add MCP Screen (Registry Unavailable)

```
+------------------------------------------+
| < Add MCP                                 |
+------------------------------------------+
|                                          |
| URL: [________________________________]  |
|                                          |
| -- or search registry --                 |
|                                          |
| Registry: [Smithery               v]     |
| Search:   [github_________________]      |
|                                          |
| +--------------------------------------+ |
| |                                      | |
| |   [!] Registry temporarily           | |
| |   unavailable.                       | |
| |                                      | |
| |   Try again later or paste a         | |
| |   URL directly.                      | |
| |                                      | |
| +--------------------------------------+ |
|                                          |
|                                  [Next]  |
+------------------------------------------+
```

---

## Add MCP Screen (With Search Results)

```
+------------------------------------------+
| < Add MCP                                 |
+------------------------------------------+
|                                          |
| URL: [________________________________]  |
|                                          |
| -- or search registry --                 |
|                                          |
| Registry: [Both                   v]     |
| Search:   [github_________________]      |
|                                          |
| +--------------------------------------+ |
| | > GitHub                  [Official] | |
| |   Manage repos, issues, PRs...       | |
| +--------------------------------------+ |
| |   GitHub                  [Smithery] | |
| |   GitHub integration for AI...       | |
| +--------------------------------------+ |
| |   GitHub Gist             [Smithery] | |
| |   Create and manage gists...         | |
| +--------------------------------------+ |
|                                          |
|                                  [Next]  |
+------------------------------------------+
```

- `>` = selected row
- [Official] or [Smithery] = source badge
- Click row to select, then Next

---

## Configure: API Key / PAT Auth

```
+------------------------------------------+
| < Configure: GitHub                       |
+------------------------------------------+
|                                          |
| Name: [GitHub_________________________]  |
|                                          |
| This MCP requires authentication.        |
|                                          |
| (*) API Key / PAT:                       |
|     [________________________________]   |
|                                          |
| ( ) Keyfile Path:                        |
|     [________________________________]   |
|     (e.g. ~/.github_token)               |
|                                          |
|                          [Cancel] [Save] |
+------------------------------------------+
```

- Radio: choose API key OR keyfile path
- API Key: paste actual token
- Keyfile: paste path like `~/.github_token`

---

## Configure: OAuth Auth (Not Connected)

```
+------------------------------------------+
| < Configure: GitHub                       |
+------------------------------------------+
|                                          |
| Name: [GitHub_________________________]  |
|                                          |
| This MCP requires OAuth authentication.  |
|                                          |
|      [  Authorize with GitHub  ]         |
|                                          |
| Status: Not connected                    |
|                                          |
|                          [Cancel] [Save] |
+------------------------------------------+
```

- Button opens browser for OAuth flow
- Save disabled until authorized

---

## Configure: OAuth Auth (Connected)

```
+------------------------------------------+
| < Configure: GitHub                       |
+------------------------------------------+
|                                          |
| Name: [GitHub_________________________]  |
|                                          |
| This MCP requires OAuth authentication.  |
|                                          |
|      [  Reauthorize with GitHub  ]       |
|                                          |
| Status: Connected as @acoliver           |
|                                          |
|                          [Cancel] [Save] |
+------------------------------------------+
```

- Shows connected status with username
- Can reauthorize if needed

---

## Configure: No Auth Required

```
+------------------------------------------+
| < Configure: Filesystem                   |
+------------------------------------------+
|                                          |
| Name: [Filesystem____________________]   |
|                                          |
| No authentication required.              |
|                                          |
|                          [Cancel] [Save] |
+------------------------------------------+
```

---

## Configure: Multiple Credentials Required

For MCPs like AWS that need multiple env vars:

```
+------------------------------------------+
| < Configure: AWS S3                       |
+------------------------------------------+
|                                          |
| Name: [AWS S3________________________]   |
|                                          |
| This MCP requires authentication.        |
|                                          |
| AWS_ACCESS_KEY_ID: (required)            |
| [________________________________]       |
|                                          |
| AWS_SECRET_ACCESS_KEY: (required)        |
| [________________________________]       |
|                                          |
| AWS_REGION: (optional)                   |
| [us-east-1________________________]      |
|                                          |
|                          [Cancel] [Save] |
+------------------------------------------+
```

- Fields generated from MCP's `environmentVariables` array
- Required fields marked, optional fields have defaults
- Each credential stored separately

---

## Configure: With Custom Settings

```
+------------------------------------------+
| < Configure: Filesystem                   |
+------------------------------------------+
|                                          |
| Name: [Filesystem____________________]   |
|                                          |
| No authentication required.              |
|                                          |
| Allowed Paths:                           |
| +--------------------------------------+ |
| | ~/Documents                      [-] | |
| | ~/Downloads                      [-] | |
| | [+ Add Path]                         | |
| +--------------------------------------+ |
|                                          |
|                          [Cancel] [Save] |
+------------------------------------------+
```

- Settings generated from MCP's configSchema
- Dynamic fields based on what MCP requires

---

## Delete Confirmation

```
+------------------------------------------+
|     Delete "GitHub"?                     |
|                                          |
|  This will remove the MCP and its        |
|  stored credentials.                     |
|                                          |
|              [Cancel]  [Delete]          |
+------------------------------------------+
```

---

## Settings: MCP Status Indicators

```
+--------------------------------------+
| [x] GitHub              [Connected]  |
| [ ] Filesystem          [Idle]       |
| [x] Brave Search        [Error]      |
+--------------------------------------+
```

Status badges:
- **Connected** (green): Currently running
- **Idle** (gray): Enabled but not spawned yet
- **Error** (red): Failed to start or crashed
- **Disabled** (no badge): Checkbox unchecked

---

## Chat View: Tool Call In Progress

```
+----------------------------------------------------------+
| User: Search GitHub for rust MCP libraries               |
+----------------------------------------------------------+
|                                                          |
| [spinner] Using github.search_repositories...            |
| +------------------------------------------------------+ |
| | query: "rust MCP libraries"                          | |
| | sort: "stars"                                        | |
| +------------------------------------------------------+ |
|                                                          |
+----------------------------------------------------------+
```

- Spinner indicates tool is running
- Shows tool name with MCP prefix
- Shows parameters passed to tool

---

## Chat View: Tool Call Success

```
+----------------------------------------------------------+
| User: Search GitHub for rust MCP libraries               |
+----------------------------------------------------------+
|                                                          |
| [check] github.search_repositories completed             |
|                                                          |
| Found 12 repositories:                                   |
|                                                          |
| 1. **anthropics/mcp** (2.3k stars)                       |
|    Official MCP protocol implementation                  |
|                                                          |
| 2. **serdes-ai/mcp** (156 stars)                         |
|    Rust MCP client and toolset                           |
|                                                          |
| 3. **example/mcp-tools** (89 stars)                      |
|    Collection of MCP tools                               |
|                                                          |
+----------------------------------------------------------+
| Assistant:                                               |
|                                                          |
| I found several Rust MCP libraries. The most popular     |
| is **anthropics/mcp** with 2.3k stars - it's the         |
| official implementation. For a Rust-native client,       |
| **serdes-ai/mcp** looks promising with 156 stars.        |
|                                                          |
| Would you like more details on any of these?             |
+----------------------------------------------------------+
```

- Green check indicates success
- Tool results shown in collapsible block
- Agent synthesizes results in natural language below

---

## Chat View: Tool Call Error

```
+----------------------------------------------------------+
| User: Create a GitHub issue in my private repo           |
+----------------------------------------------------------+
|                                                          |
| [x] github.create_issue failed                           |
| +------------------------------------------------------+ |
| | Error: Resource not accessible by integration        | |
| | The token doesn't have access to this repository.    | |
| +------------------------------------------------------+ |
|                                                          |
+----------------------------------------------------------+
| Assistant:                                               |
|                                                          |
| I couldn't create the issue - the GitHub token doesn't   |
| have access to that repository. You may need to:         |
|                                                          |
| 1. Use a token with `repo` scope for private repos       |
| 2. Or add the app to the repository's allowed list       |
|                                                          |
| Would you like me to help you generate a new token?      |
+----------------------------------------------------------+
```

- Red X indicates failure
- Error message from MCP shown
- Agent explains and suggests remediation

---

## Chat View: Tool Call Timeout

```
+----------------------------------------------------------+
| User: Analyze this large codebase                        |
+----------------------------------------------------------+
|                                                          |
| [clock] github.get_repository_content timed out          |
| +------------------------------------------------------+ |
| | Timeout after 30 seconds                             | |
| +------------------------------------------------------+ |
|                                                          |
+----------------------------------------------------------+
| Assistant:                                               |
|                                                          |
| The request timed out - the repository might be too      |
| large to analyze in one request. Would you like me to:   |
|                                                          |
| 1. Try a specific directory instead?                     |
| 2. Retry with a longer timeout?                          |
+----------------------------------------------------------+
```

---

## Toast Notifications

**MCP Disconnected:**
```
+------------------------------------------+
| [!] GitHub MCP disconnected.             |
|     Reconnecting...                [x]   |
+------------------------------------------+
```

**MCP Restart Failed:**
```
+------------------------------------------+
| [x] GitHub MCP failed to restart.        |
|     Check settings.                [x]   |
+------------------------------------------+
```

**MCP Connected:**
```
+------------------------------------------+
| [check] GitHub MCP connected.      [x]   |
+------------------------------------------+
```
