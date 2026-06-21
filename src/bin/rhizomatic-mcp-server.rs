use rhizomatic_server::{
    mcp_client::RhizomaticApiClient,
    models::{CreateThemagraph, UpdateThemagraph},
};
use rmcp::{ServiceExt, handler::server::wrapper::Parameters, tool, tool_router, transport::stdio};
use schemars::JsonSchema;
use serde::Deserialize;
use std::env;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Debug, Clone)]
struct RhizomaticMcpServer {
    api: RhizomaticApiClient,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AuthParams {
    /// Path to a file containing the API token used to authenticate against the rhizomatic HTTP server.
    api_token_file: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetThemagraphParams {
    /// Path to a file containing the API token used to authenticate against the rhizomatic HTTP server.
    api_token_file: String,
    /// Themagraph identifier.
    id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CreateThemagraphParams {
    /// Path to a file containing the API token used to authenticate against the rhizomatic HTTP server.
    api_token_file: String,
    /// Themagraph body text.
    body: String,
    /// Explicit intralinks to associate with the themagraph.
    #[serde(default)]
    links: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct UpdateThemagraphParams {
    /// Path to a file containing the API token used to authenticate against the rhizomatic HTTP server.
    api_token_file: String,
    /// Themagraph identifier.
    id: String,
    /// Replacement themagraph body text.
    body: String,
    /// Explicit intralinks to associate with the themagraph.
    #[serde(default)]
    links: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct QueryThemagraphsParams {
    /// Path to a file containing the API token used to authenticate against the rhizomatic HTTP server.
    api_token_file: String,
    /// Rhizomatic query string.
    query: String,
}

#[tool_router(server_handler)]
impl RhizomaticMcpServer {
    #[tool(
        name = "rhizomatic_api_health",
        description = "Check the health of the rhizomatic HTTP API."
    )]
    async fn api_health(
        &self,
        Parameters(params): Parameters<AuthParams>,
    ) -> Result<String, String> {
        self.api
            .health(params.api_token_file)
            .await
            .and_then(to_json_string)
    }

    #[tool(
        name = "list_themagraphs",
        description = "List all themagraphs from the rhizomatic HTTP API."
    )]
    async fn list_themagraphs(
        &self,
        Parameters(params): Parameters<AuthParams>,
    ) -> Result<String, String> {
        let themagraphs = self.api.list_themagraphs(params.api_token_file).await?;
        to_json_string(themagraphs)
    }

    #[tool(
        name = "get_themagraph",
        description = "Fetch a single themagraph by id from the rhizomatic HTTP API."
    )]
    async fn get_themagraph(
        &self,
        Parameters(params): Parameters<GetThemagraphParams>,
    ) -> Result<String, String> {
        let themagraph = self
            .api
            .get_themagraph(params.api_token_file, &params.id)
            .await?;
        to_json_string(themagraph)
    }

    #[tool(
        name = "create_themagraph",
        description = "Create a themagraph through the rhizomatic HTTP API."
    )]
    async fn create_themagraph(
        &self,
        Parameters(params): Parameters<CreateThemagraphParams>,
    ) -> Result<String, String> {
        let themagraph = self
            .api
            .create_themagraph(
                params.api_token_file,
                &CreateThemagraph {
                    body: params.body,
                    links: params.links,
                },
            )
            .await?;
        to_json_string(themagraph)
    }

    #[tool(
        name = "update_themagraph",
        description = "Update an existing themagraph through the rhizomatic HTTP API."
    )]
    async fn update_themagraph(
        &self,
        Parameters(params): Parameters<UpdateThemagraphParams>,
    ) -> Result<String, String> {
        let themagraph = self
            .api
            .update_themagraph(
                params.api_token_file,
                &params.id,
                &UpdateThemagraph {
                    body: params.body,
                    links: params.links,
                },
            )
            .await?;
        to_json_string(themagraph)
    }

    #[tool(
        name = "delete_themagraph",
        description = "Delete a themagraph through the rhizomatic HTTP API."
    )]
    async fn delete_themagraph(
        &self,
        Parameters(params): Parameters<GetThemagraphParams>,
    ) -> Result<String, String> {
        let value = self
            .api
            .delete_themagraph(params.api_token_file, &params.id)
            .await?;
        to_json_string(value)
    }

    #[tool(
        name = "query_themagraphs",
        description = "Run a rhizomatic query against the rhizomatic HTTP API."
    )]
    async fn query_themagraphs(
        &self,
        Parameters(params): Parameters<QueryThemagraphsParams>,
    ) -> Result<String, String> {
        let response = self
            .api
            .query_themagraphs(params.api_token_file, &params.query)
            .await?;
        to_json_string(response)
    }
}

fn to_json_string<T: serde::Serialize>(value: T) -> Result<String, String> {
    serde_json::to_string(&value)
        .map_err(|error| format!("failed to serialize tool result: {error}"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("rhizomatic_mcp_server=info")),
        )
        .init();

    let server_url = parse_server_url(env::args_os())?;
    let service = RhizomaticMcpServer {
        api: RhizomaticApiClient::new(server_url),
    }
    .serve(stdio())
    .await?;

    service.waiting().await?;
    Ok(())
}

fn parse_server_url(
    mut args: impl Iterator<Item = impl Into<std::ffi::OsString>>,
) -> Result<String, Box<dyn std::error::Error>> {
    let _program = args.next();
    let mut server_url = "http://127.0.0.1:3000".to_owned();

    while let Some(flag) = args.next() {
        let flag = flag.into();
        if flag == "--server-url" {
            let Some(value) = args.next() else {
                return Err("missing URL after --server-url".into());
            };
            server_url = value.into().to_string_lossy().into_owned();
        } else {
            return Err(format!("unknown argument: {}", flag.to_string_lossy()).into());
        }
    }

    Ok(server_url)
}
