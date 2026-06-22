use crate::models::{
    CreateTag, CreateThemagraph, QueryRequest, QueryResponse, RegexQueryRequest, Tag, Themagraph,
    UpdateThemagraph,
};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use std::{fs, path::Path};

#[derive(Debug, Clone)]
pub struct RhizomaticApiClient {
    base_url: String,
    client: Client,
}

impl RhizomaticApiClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            client: Client::new(),
        }
    }

    pub async fn health(&self, token_file: impl AsRef<Path>) -> Result<Value, String> {
        self.get_json("/api/health", token_file).await
    }

    pub async fn list_themagraphs(
        &self,
        token_file: impl AsRef<Path>,
    ) -> Result<Vec<Themagraph>, String> {
        self.request(
            self.client
                .get(format!("{}/api/themagraphs", self.base_url)),
            token_file,
        )
        .await?
        .json::<Vec<Themagraph>>()
        .await
        .map_err(|error| format!("failed to decode themagraph list: {error}"))
    }

    pub async fn get_themagraph(
        &self,
        token_file: impl AsRef<Path>,
        id: &str,
    ) -> Result<Themagraph, String> {
        self.request(
            self.client
                .get(format!("{}/api/themagraphs/{id}", self.base_url)),
            token_file,
        )
        .await?
        .json::<Themagraph>()
        .await
        .map_err(|error| format!("failed to decode themagraph: {error}"))
    }

    pub async fn create_themagraph(
        &self,
        token_file: impl AsRef<Path>,
        payload: &CreateThemagraph,
    ) -> Result<Themagraph, String> {
        self.request(
            self.client
                .post(format!("{}/api/themagraphs", self.base_url))
                .json(payload),
            token_file,
        )
        .await?
        .json::<Themagraph>()
        .await
        .map_err(|error| format!("failed to decode created themagraph: {error}"))
    }

    pub async fn update_themagraph(
        &self,
        token_file: impl AsRef<Path>,
        id: &str,
        payload: &UpdateThemagraph,
    ) -> Result<Themagraph, String> {
        self.request(
            self.client
                .put(format!("{}/api/themagraphs/{id}", self.base_url))
                .json(payload),
            token_file,
        )
        .await?
        .json::<Themagraph>()
        .await
        .map_err(|error| format!("failed to decode updated themagraph: {error}"))
    }

    pub async fn delete_themagraph(
        &self,
        token_file: impl AsRef<Path>,
        id: &str,
    ) -> Result<Value, String> {
        let response = self
            .request(
                self.client
                    .delete(format!("{}/api/themagraphs/{id}", self.base_url)),
                token_file,
            )
            .await?;
        Ok(json!({
            "deleted": response.status() == StatusCode::NO_CONTENT,
            "status": response.status().as_u16(),
        }))
    }

    pub async fn query_themagraphs(
        &self,
        token_file: impl AsRef<Path>,
        query: &str,
    ) -> Result<QueryResponse, String> {
        self.request(
            self.client
                .post(format!("{}/api/query", self.base_url))
                .json(&QueryRequest {
                    query: query.to_owned(),
                }),
            token_file,
        )
        .await?
        .json::<QueryResponse>()
        .await
        .map_err(|error| format!("failed to decode query response: {error}"))
    }

    pub async fn query_themagraphs_regex(
        &self,
        token_file: impl AsRef<Path>,
        payload: &RegexQueryRequest,
    ) -> Result<QueryResponse, String> {
        self.request(
            self.client
                .post(format!("{}/api/query/regex", self.base_url))
                .json(payload),
            token_file,
        )
        .await?
        .json::<QueryResponse>()
        .await
        .map_err(|error| format!("failed to decode regex query response: {error}"))
    }

    pub async fn get_themagraph_by_uuid(
        &self,
        token_file: impl AsRef<Path>,
        id: &str,
    ) -> Result<Themagraph, String> {
        self.request(
            self.client
                .get(format!("{}/api/themagraphs/uuid/{id}", self.base_url)),
            token_file,
        )
        .await?
        .json::<Themagraph>()
        .await
        .map_err(|error| format!("failed to decode themagraph by UUID: {error}"))
    }

    pub async fn list_tags(&self, token_file: impl AsRef<Path>) -> Result<Vec<Tag>, String> {
        self.request(
            self.client.get(format!("{}/api/tags", self.base_url)),
            token_file,
        )
        .await?
        .json::<Vec<Tag>>()
        .await
        .map_err(|error| format!("failed to decode tag list: {error}"))
    }

    pub async fn query_tags_regex(
        &self,
        token_file: impl AsRef<Path>,
        payload: &RegexQueryRequest,
    ) -> Result<Vec<Tag>, String> {
        self.request(
            self.client
                .post(format!("{}/api/tags/query/regex", self.base_url))
                .json(payload),
            token_file,
        )
        .await?
        .json::<Vec<Tag>>()
        .await
        .map_err(|error| format!("failed to decode regex tag query response: {error}"))
    }

    pub async fn create_tag(
        &self,
        token_file: impl AsRef<Path>,
        payload: &CreateTag,
    ) -> Result<Tag, String> {
        self.request(
            self.client
                .post(format!("{}/api/tags", self.base_url))
                .json(payload),
            token_file,
        )
        .await?
        .json::<Tag>()
        .await
        .map_err(|error| format!("failed to decode created tag: {error}"))
    }

    async fn get_json(&self, path: &str, token_file: impl AsRef<Path>) -> Result<Value, String> {
        self.request(
            self.client.get(format!("{}{}", self.base_url, path)),
            token_file,
        )
        .await?
        .json::<Value>()
        .await
        .map_err(|error| format!("failed to decode JSON response: {error}"))
    }

    async fn request(
        &self,
        builder: reqwest::RequestBuilder,
        token_file: impl AsRef<Path>,
    ) -> Result<reqwest::Response, String> {
        let token = read_api_token_file(token_file)?;
        let response = builder
            .bearer_auth(token)
            .send()
            .await
            .map_err(|error| format!("request failed: {error}"))?;

        if response.status().is_success() {
            Ok(response)
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read response body>".to_owned());
            Err(format!(
                "API request failed with status {}: {}",
                status.as_u16(),
                body
            ))
        }
    }
}

pub fn read_api_token_file(path: impl AsRef<Path>) -> Result<String, String> {
    let path = path.as_ref();
    let token = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read API token file '{}': {error}",
            path.display()
        )
    })?;
    let token = token.trim().to_owned();
    if token.is_empty() {
        return Err(format!(
            "API token file '{}' did not contain a token",
            path.display()
        ));
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::read_api_token_file;
    use std::{fs, path::PathBuf};

    #[test]
    fn trims_token_file_contents() {
        let path = PathBuf::from("/tmp/rhizomatic-server-token-test.txt");
        fs::write(&path, " test-token \n").expect("token file should be written");
        let token = read_api_token_file(&path).expect("token should be read");
        assert_eq!(token, "test-token");
        let _ = fs::remove_file(path);
    }
}
