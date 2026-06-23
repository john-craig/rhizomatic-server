use crate::config::Config;
use crate::models::{
    CreateTag, CreateThemagraph, DetailTemplate, IndexTemplate, QueryRequest, QueryResponse,
    RegexQueryRequest, SearchParams, SearchResult, Tag, ThemagraphForm, UpdateThemagraph,
};
use crate::query::{filter_themagraphs, regex_filter_tags, regex_filter_themagraphs};
use crate::store::Store;
use askama::Template;
use axum::{
    Form, Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde_json::json;
use std::{env, net::SocketAddr, sync::Arc};
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub api_token: String,
}

pub async fn serve(store: Store, config: Config) -> Result<(), std::io::Error> {
    let bind_address: SocketAddr = config.bind_address;
    let state = Arc::new(AppState {
        store,
        api_token: config.api_token,
    });
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(bind_address).await?;
    info!("listening on http://{bind_address}");
    axum::serve(listener, app).await
}

pub fn router(state: Arc<AppState>) -> Router {
    let static_dir = static_dir();
    let api = Router::new()
        .route("/health", get(health))
        .route(
            "/themagraphs",
            get(list_themagraphs).post(create_themagraph),
        )
        .route(
            "/themagraphs/{id}",
            get(get_themagraph)
                .put(update_themagraph)
                .delete(delete_themagraph),
        )
        .route(
            "/query",
            get(query_themagraphs).post(query_themagraphs_post),
        )
        .route("/query/regex", post(query_themagraphs_regex))
        .route("/themagraphs/uuid/{id}", get(get_themagraph_by_uuid))
        .route("/tags", get(list_tags).post(create_tag))
        .route("/tags/query/regex", post(query_tags_regex))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_token,
        ));

    Router::new()
        .route("/", get(index))
        .route("/themagraphs/{id}", get(detail))
        .route("/ui/themagraphs", post(create_from_form))
        .route("/ui/themagraphs/{id}", post(update_from_form))
        .route("/ui/themagraphs/{id}/delete", post(delete_from_form))
        .nest("/api", api)
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn static_dir() -> String {
    env::var("RHIZOMATIC_STATIC_DIR").unwrap_or_else(|_| "static".to_owned())
}

async fn require_api_token(
    State(state): State<Arc<AppState>>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_bearer_token);
    let header_token = request
        .headers()
        .get("x-api-token")
        .and_then(|value| value.to_str().ok());

    let provided = bearer.or(header_token);
    if provided == Some(state.api_token.as_str()) {
        return next.run(request).await;
    }

    AppError::Unauthorized("missing or invalid API token".to_owned()).into_response()
}

fn parse_bearer_token(value: &str) -> Option<&str> {
    value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

async fn index(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>, AppError> {
    let themagraphs = state.store.list_themagraphs().await?;
    let matches: Vec<SearchResult> = match params
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
    {
        Some(query) => {
            let matched_ids = filter_themagraphs(&themagraphs, query)
                .into_iter()
                .map(|themagraph| themagraph.id.clone())
                .collect::<std::collections::HashSet<_>>();
            themagraphs
                .into_iter()
                .filter(|themagraph| matched_ids.contains(&themagraph.id))
                .map(|themagraph| SearchResult {
                    themagraph,
                    matched_by_query: true,
                })
                .collect()
        }
        None => themagraphs
            .into_iter()
            .map(|themagraph| SearchResult {
                themagraph,
                matched_by_query: false,
            })
            .collect(),
    };

    render_template(IndexTemplate {
        query: params.query.unwrap_or_default(),
        total_themagraphs: matches.len(),
        themagraphs: matches,
    })
}

async fn detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppError> {
    let Some(themagraph) = state.store.get_themagraph(&id).await? else {
        return Err(AppError::NotFound(format!("themagraph '{id}' not found")));
    };

    render_template(DetailTemplate {
        links_text: themagraph.links.join(", "),
        themagraph,
    })
}

async fn create_from_form(
    State(state): State<Arc<AppState>>,
    Form(form): Form<ThemagraphForm>,
) -> Result<Redirect, AppError> {
    let links = form.parsed_links();
    let themagraph = state
        .store
        .create_themagraph(CreateThemagraph {
            body: form.body,
            links,
        })
        .await?;
    Ok(Redirect::to(&format!("/themagraphs/{}", themagraph.id)))
}

async fn update_from_form(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Form(form): Form<ThemagraphForm>,
) -> Result<Redirect, AppError> {
    let links = form.parsed_links();
    let Some(themagraph) = state
        .store
        .update_themagraph(
            &id,
            UpdateThemagraph {
                body: form.body,
                links,
            },
        )
        .await?
    else {
        return Err(AppError::NotFound(format!("themagraph '{id}' not found")));
    };
    Ok(Redirect::to(&format!("/themagraphs/{}", themagraph.id)))
}

async fn delete_from_form(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Redirect, AppError> {
    if !state.store.delete_themagraph(&id).await? {
        return Err(AppError::NotFound(format!("themagraph '{id}' not found")));
    }
    Ok(Redirect::to("/"))
}

async fn list_themagraphs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::models::Themagraph>>, AppError> {
    Ok(Json(state.store.list_themagraphs().await?))
}

async fn get_themagraph(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::Themagraph>, AppError> {
    let Some(themagraph) = state.store.get_themagraph(&id).await? else {
        return Err(AppError::NotFound(format!("themagraph '{id}' not found")));
    };
    Ok(Json(themagraph))
}

async fn get_themagraph_by_uuid(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::Themagraph>, AppError> {
    get_themagraph(State(state), Path(id)).await
}

async fn create_themagraph(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateThemagraph>,
) -> Result<(StatusCode, Json<crate::models::Themagraph>), AppError> {
    let themagraph = state.store.create_themagraph(payload).await?;
    Ok((StatusCode::CREATED, Json(themagraph)))
}

async fn list_tags(State(state): State<Arc<AppState>>) -> Result<Json<Vec<Tag>>, AppError> {
    Ok(Json(state.store.list_tags().await?))
}

async fn create_tag(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTag>,
) -> Result<(StatusCode, Json<Tag>), AppError> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest(
            "tag name must not be empty".to_owned(),
        ));
    }
    let tag = state
        .store
        .create_tag(CreateTag {
            name: name.to_owned(),
        })
        .await?;
    Ok((StatusCode::CREATED, Json(tag)))
}

async fn update_themagraph(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateThemagraph>,
) -> Result<Json<crate::models::Themagraph>, AppError> {
    let Some(themagraph) = state.store.update_themagraph(&id, payload).await? else {
        return Err(AppError::NotFound(format!("themagraph '{id}' not found")));
    };
    Ok(Json(themagraph))
}

async fn delete_themagraph(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    if state.store.delete_themagraph(&id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("themagraph '{id}' not found")))
    }
}

async fn query_themagraphs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<QueryResponse>, AppError> {
    let query = params.query.unwrap_or_default();
    run_query(&state.store, query).await
}

async fn query_themagraphs_post(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, AppError> {
    run_query(&state.store, payload.query).await
}

async fn query_themagraphs_regex(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegexQueryRequest>,
) -> Result<Json<QueryResponse>, AppError> {
    let target = payload.target.as_deref().unwrap_or("any");
    if !matches!(target, "any" | "body" | "links") {
        return Err(AppError::BadRequest(
            "regex query target must be one of: any, body, links".to_owned(),
        ));
    }

    let themagraphs = state.store.list_themagraphs().await?;
    let matches = regex_filter_themagraphs(
        &themagraphs,
        &payload.pattern,
        payload.case_insensitive,
        Some(target),
    )
    .map_err(|error| AppError::BadRequest(error.message))?
    .into_iter()
    .cloned()
    .collect();

    Ok(Json(QueryResponse {
        query: payload.pattern,
        matches,
    }))
}

async fn query_tags_regex(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegexQueryRequest>,
) -> Result<Json<Vec<Tag>>, AppError> {
    let tags = state.store.list_tags().await?;
    let tag_names = tags.iter().map(|tag| tag.name.clone()).collect::<Vec<_>>();
    let matched_names = regex_filter_tags(&tag_names, &payload.pattern, payload.case_insensitive)
        .map_err(|error| AppError::BadRequest(error.message))?
        .into_iter()
        .map(|tag| tag.as_str())
        .collect::<std::collections::HashSet<_>>();

    Ok(Json(
        tags.into_iter()
            .filter(|tag| matched_names.contains(tag.name.as_str()))
            .collect(),
    ))
}

async fn run_query(store: &Store, query: String) -> Result<Json<QueryResponse>, AppError> {
    let themagraphs = store.list_themagraphs().await?;
    let matches = filter_themagraphs(&themagraphs, &query)
        .into_iter()
        .cloned()
        .collect();
    Ok(Json(QueryResponse { query, matches }))
}

fn render_template(template: impl Template) -> Result<Html<String>, AppError> {
    template
        .render()
        .map(Html)
        .map_err(|error| AppError::Internal(error.to_string()))
}

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Internal(String),
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        Self::Internal(value.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::Unauthorized(message) => (StatusCode::UNAUTHORIZED, message),
            Self::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };
        let mut response = (
            status,
            Json(json!({
                "error": message,
            })),
        )
            .into_response();
        if status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                header::HeaderValue::from_static("Bearer"),
            );
        }
        response
    }
}
