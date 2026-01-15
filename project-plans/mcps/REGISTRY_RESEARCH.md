# MCP Registry Research

This document contains API research for MCP registries.
For full specification, see [SPEC.md](./SPEC.md).

---

## Registry Comparison

| Registry | Search API | Details API | Auth Detection | Notes |
|----------|------------|-------------|----------------|-------|
| Official | Yes | Via search | `environmentVariables` | Authoritative, package info |
| Smithery | Yes | Yes | `configSchema` | Rich metadata, tools list |
| Glama | No | No | N/A | Returns HTML, skip |

---

## Official MCP Registry

**Base URL:** `https://registry.modelcontextprotocol.io`

### Search Endpoint

```
GET /v0.1/servers?search={query}&limit={n}
```

**Parameters:**
- `search` - substring match on name
- `limit` - max results (default 30, max 100)
- `cursor` - pagination cursor

**Response:**
```json
{
  "servers": [{
    "server": {
      "name": "io.github.owner/server-name",
      "description": "Server description",
      "title": "Friendly Name",
      "repository": { "url": "https://github.com/...", "source": "github" },
      "version": "1.0.2",
      "packages": [{
        "registryType": "npm",
        "identifier": "@org/package-name",
        "version": "1.0.2",
        "runtimeHint": "npx",
        "transport": { "type": "stdio" },
        "environmentVariables": [
          {
            "name": "GITHUB_TOKEN",
            "description": "GitHub personal access token",
            "isRequired": true,
            "isSecret": true
          }
        ]
      }]
    },
    "_meta": {
      "io.modelcontextprotocol.registry/official": {
        "status": "active",
        "publishedAt": "2025-12-13T07:28:11Z"
      }
    }
  }],
  "metadata": { "nextCursor": "...", "count": 3 }
}
```

**Auth Detection from `environmentVariables`:**
- `isSecret: true` = credential required
- Name patterns: `*_TOKEN`, `*_PAT`, `*_API_KEY` = API key auth
- `*_CLIENT_ID` + `*_CLIENT_SECRET` = OAuth

---

## Smithery Registry

**Base URL:** `https://registry.smithery.ai`

### Search Endpoint

```
GET /servers?q={query}&pageSize={n}
```

**Parameters:**
- `q` - semantic search (name, description, tags)
- `pageSize` - max results

**Response:**
```json
{
  "servers": [{
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "qualifiedName": "@owner/server-name",
    "displayName": "Friendly Name",
    "description": "Server description",
    "iconUrl": "https://...",
    "verified": true,
    "useCount": 11451,
    "remote": true,
    "isDeployed": true,
    "createdAt": "2025-10-07T07:56:58Z",
    "homepage": "https://smithery.ai/server/@owner/server-name"
  }]
}
```

### Details Endpoint

```
GET /servers/{qualifiedName}
```

**Response:**
```json
{
  "qualifiedName": "@owner/server-name",
  "displayName": "Friendly Name",
  "description": "Server description",
  "deploymentUrl": "https://server.smithery.ai/@owner/server-name",
  "connections": [{
    "type": "http",
    "deploymentUrl": "https://server.smithery.ai/@owner/server-name/mcp",
    "configSchema": {
      "type": "object",
      "properties": {
        "allowedPaths": {
          "type": "array",
          "items": { "type": "string" }
        }
      }
    }
  }],
  "tools": [{
    "name": "read_file",
    "description": "Read contents of a file",
    "inputSchema": {
      "type": "object",
      "required": ["path"],
      "properties": {
        "path": { "type": "string" }
      }
    }
  }],
  "security": null
}
```

---

## Search Merge Strategy

1. Fire parallel requests to both registries
2. Tag each result with source: `[Official]` or `[Smithery]`
3. Dedupe by similar names (fuzzy match)
4. Sort by: verified first, then useCount/popularity
5. Display merged list

---

## Test Commands

**Official Registry:**
```bash
# Search
curl -s "https://registry.modelcontextprotocol.io/v0.1/servers?search=github&limit=3" | jq '.servers[].server.name'

# With env vars
curl -s "https://registry.modelcontextprotocol.io/v0.1/servers?search=github&limit=1" | jq '.servers[0].server.packages[0].environmentVariables'
```

**Smithery Registry:**
```bash
# Search
curl -s "https://registry.smithery.ai/servers?q=github&pageSize=3" | jq '.servers[].displayName'

# Details
curl -s "https://registry.smithery.ai/servers/github" | jq '{tools: .tools[].name, connections: .connections}'
```
