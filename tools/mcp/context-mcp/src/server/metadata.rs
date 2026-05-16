use rmcp::{
    ServerHandler,
    model::{
        ServerCapabilities,
        ServerInfo,
    },
    tool_handler,
};

use super::ContextServer;

#[tool_handler]
impl ServerHandler for ContextServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Context Engine MCP Server — manages hypergraph workspaces.\n\n\
                 **Start here:** Call the `help` tool to discover available commands,\n\
                 or the `workflow` tool for ready-to-use command sequences.\n\n\
                 Then use the `execute` tool with a command JSON object.\n\n\
                 Quick start:\n\
                 1. Call `workflow` with name=\"basic\" to get a step-by-step template\n\
                 2. Follow the steps using the `execute` tool\n\
                 3. Call `help` with command=\"<name>\" for detailed parameter info\n\n\
                 Or dive straight in:\n\
                 1. {\"command\": \"create_workspace\", \"name\": \"demo\"}\n\
                 2. {\"command\": \"add_atoms\", \"workspace\": \"demo\", \"chars\": [\"a\",\"b\",\"c\"]}\n\
                 3. {\"command\": \"insert_sequence\", \"workspace\": \"demo\", \"text\": \"abc\"}\n\
                 4. {\"command\": \"search_sequence\", \"workspace\": \"demo\", \"text\": \"ab\"}\n\
                 5. {\"command\": \"save_workspace\", \"name\": \"demo\"}"
                    .to_string(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}