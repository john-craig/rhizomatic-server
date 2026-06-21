use crate::models::{
    CreateThemagraph, DetailTemplate, IndexTemplate, QueryRequest, QueryResponse, SearchParams,
    SearchResult, ThemagraphForm, UpdateThemagraph,
};
use crate::query::filter_themagraphs;
use crate::store::Store;
use askama::Template;
use axum::{
    Form, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde_json::json;
use std::{net::SocketAddr, sync::Arc};
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
}

pub async fn serve(store: Store, bind_address: SocketAddr) -> Result<(), std::io::Error> {
    let state = Arc::new(AppState { store });
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(bind_address).await?;
    info!("listening on http://{bind_address}");
    axum::serve(listener, app).await
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/themagraphs/{id}", get(detail))
        .route("/ui/themagraphs", post(create_from_form))
        .route("/ui/themagraphs/{id}", post(update_from_form))
        .route("/ui/themagraphs/{id}/delete", post(delete_from_form))
        .route("/api/health", get(health))
        .route(
            "/api/themagraphs",
            get(list_themagraphs).post(create_themagraph),
        )
        .route(
            "/api/themagraphs/{id}",
            get(get_themagraph)
                .put(update_themagraph)
                .delete(delete_themagraph),
        )
        .route(
            "/api/query",
            get(query_themagraphs).post(query_themagraphs_post),
        )
        .nest_service("/static", ServeDir::new("static"))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
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

async fn create_themagraph(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateThemagraph>,
) -> Result<(StatusCode, Json<crate::models::Themagraph>), AppError> {
    let themagraph = state.store.create_themagraph(payload).await?;
    Ok((StatusCode::CREATED, Json(themagraph)))
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
            Self::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };

        (
            status,
            Json(json!({
                "error": message,
            })),
        )
            .into_response()
    }
}
