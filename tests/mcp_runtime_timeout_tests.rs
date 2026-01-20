use personal_agent::mcp::McpRuntime;

struct HangingTransport;

#[async_trait::async_trait]
impl serdes_ai::mcp::McpTransport for HangingTransport {
    async fn request(
        &self,
        _request: &serdes_ai::mcp::types::JsonRpcRequest,
    ) -> serdes_ai::mcp::error::McpResult<serdes_ai::mcp::types::JsonRpcResponse> {
        futures::future::pending().await
    }

    async fn notify(
        &self,
        _notification: &serdes_ai::mcp::types::JsonRpcNotification,
    ) -> serdes_ai::mcp::error::McpResult<()> {
        Ok(())
    }

    async fn close(&self) -> serdes_ai::mcp::error::McpResult<()> {
        Ok(())
    }

    fn is_connected(&self) -> bool {
        true
    }
}

#[tokio::test]
async fn call_tool_times_out_and_sets_error() {
    let transport = HangingTransport;
    let mut client = serdes_ai::mcp::McpClient::new(transport);

    let result = client
        .call_tool("hang", serde_json::json!({}))
        .await;
    assert!(result.is_err());
}
