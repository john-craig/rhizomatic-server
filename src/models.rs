use askama::Template;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Themagraph {
    pub id: String,
    pub body: String,
    pub links: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateThemagraph {
    pub body: String,
    #[serde(default)]
    pub links: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateThemagraph {
    pub body: String,
    #[serde(default)]
    pub links: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    pub query: String,
    pub matches: Vec<Themagraph>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QueryRequest {
    pub query: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegexQueryRequest {
    pub pattern: String,
    #[serde(default)]
    pub case_insensitive: bool,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateTag {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchParams {
    pub query: Option<String>,
    pub tab: Option<String>,
    pub link_query: Option<String>,
    pub named_only: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemagraphForm {
    pub body: String,
    #[serde(default)]
    pub links: String,
}

impl ThemagraphForm {
    pub fn parsed_links(&self) -> Vec<String> {
        self.links
            .split([',', '\n'])
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub themagraph: Themagraph,
    pub matched_by_query: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkResult {
    pub name: String,
    pub is_named_query: bool,
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub active_tab: String,
    pub query: String,
    pub themagraphs: Vec<SearchResult>,
    pub total_themagraphs: usize,
    pub links: Vec<LinkResult>,
    pub link_query: String,
    pub named_only: bool,
}

#[derive(Template)]
#[template(path = "detail.html")]
pub struct DetailTemplate {
    pub themagraph: Themagraph,
    pub links_text: String,
}
