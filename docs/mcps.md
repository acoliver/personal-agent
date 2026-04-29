# MCP tools

MCP stands for Model Context Protocol. In Personal Agent, MCP servers provide tools that the assistant can use during a conversation, such as searching a service, reading from a data source, creating records, or calling a local utility.

MCPs are optional. Personal Agent works as a general desktop AI assistant without them, but MCP tools can make it more useful when you want the assistant to interact with external systems.

## Why configure MCP servers?

You might configure an MCP server when you want Personal Agent to:

- Search or query a service you use.
- Work with a local or remote data source.
- Trigger actions in another application.
- Use specialized tools that are not built into the base chat experience.

Each MCP server exposes its own set of tools. The exact capabilities depend on the server you add and the credentials you provide.

## Where to manage MCPs

Open **Settings**, then choose **MCP Tools**.

From there you can:

- See configured MCP servers and their status.
- Toggle an MCP server on or off.
- Add an MCP server.
- Edit an existing MCP server.
- Delete an MCP server and its stored credentials.

The MCP add flow lets you search registries or enter a manual command, package, Docker image, or URL depending on the server you want to run.

## Basic setup flow

1. Open **Settings**.
2. Choose **MCP Tools**.
3. Click **+** to add a server.
4. Search the registry or enter a manual MCP command or URL.
5. Continue to the configuration screen.
6. Review the server name, command or URL, and required environment variables.
7. Add any required credentials or API keys.
8. Save the configuration.
9. Enable the MCP and confirm it reaches a running or healthy status.

If the server requires a separate runtime such as Node.js, npx, Docker, or access to a remote URL, install or configure that dependency before enabling the MCP.

## Safety expectations

MCP tools can grant the assistant access to external systems. Treat an MCP server like any other application integration.

Before enabling a server, check:

- Who publishes or maintains it.
- What tools it exposes.
- What credentials or data it can access.
- Whether it can modify data, not just read it.
- Whether it runs local commands or connects to network services.

Use **Tool Approval** settings to control how much confirmation Personal Agent requests before using MCP tools. If you are unsure, choose a more cautious approval mode and approve each tool use only after reading the request.

## Credentials and secrets

Personal Agent stores secret values in your operating system credential store when possible. Configuration files should reference secret labels or MCP-specific secure entries rather than storing raw API keys directly.

If you delete an MCP from Settings, Personal Agent also removes the credentials associated with that MCP configuration.

## Troubleshooting

### The MCP does not start

- Verify the command, package name, Docker image, or URL.
- Ensure required runtimes are installed, such as Node.js, npx, or Docker.
- Check network connectivity if the MCP uses a remote HTTP or SSE endpoint.
- Make sure required environment variables or API keys are present.
- Toggle the MCP off and on, or restart Personal Agent.

### Authentication fails

- Re-enter the credential in the MCP configuration screen.
- Verify the API key or token is active with the provider.
- Ensure the credential has permission for the actions you are asking Personal Agent to perform.
- Check that the MCP expects the same environment variable name shown in Settings.

### Tools do not appear in chat

- Confirm the MCP is enabled.
- Confirm the server status is running or healthy.
- Ask the assistant whether MCP tools are available for the current task.
- Reopen Settings and verify the MCP remains configured after saving.

### The assistant asks for approval before using an MCP tool

This is expected when your tool approval policy requires confirmation. Review the tool name, server name, and requested action before approving.

### An MCP has too much access

Disable or delete it from **Settings > MCP Tools**. You can also adjust the service-side credentials to reduce permissions, such as using a read-only token when a server only needs read access.
