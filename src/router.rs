use std::collections::HashMap;
use std::sync::Arc;

use askama::Template;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use oauth2::{AuthorizationCode, CsrfToken, Scope};
use oauth2::reqwest::async_http_client;
use tower_sessions::Session;

use crate::InstancerState;
use crate::templating::HtmlTemplate;

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate;

pub async fn dashboard(
    session: Session,
    State(state): State<Arc<InstancerState>>
) -> Response {
    if let Ok(Some(uid)) = session.get::<String>("uid").await {
        let dashboard = DashboardTemplate;
        HtmlTemplate(dashboard).into_response()
    } else {
        Redirect::to("/login").into_response()
    }
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    oauth2_url: String
}

pub async fn login(
    session: Session,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<InstancerState>>
) -> Response {
    if let Some(code) = params.get("code") {
        match state.oauth2_client.exchange_code(AuthorizationCode::new(code.clone()))
                .request_async(async_http_client).await {
            Ok(token) => {
                session.insert("uid", "123").await.unwrap();
                Redirect::to("/").into_response()
            },
            Err(_) => StatusCode::UNAUTHORIZED.into_response()
        }
    } else {
        let (auth_url, _) = state.oauth2_client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("identify".to_string()))
            .add_scope(Scope::new("guilds".to_string()))
            .url();

        let login = LoginTemplate { oauth2_url: auth_url.to_string() };
        HtmlTemplate(login).into_response()
    }
}