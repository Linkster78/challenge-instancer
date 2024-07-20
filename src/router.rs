use std::collections::HashMap;
use std::sync::Arc;

use askama::Template;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use oauth2::{AuthorizationCode, CsrfToken, Scope, TokenResponse};
use oauth2::reqwest::async_http_client;
use tower_sessions::Session;

use crate::discord::Discord;
use crate::{discord, InstancerState};
use crate::models::{TimeSinceEpoch, User};
use crate::templating::HtmlTemplate;

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate;

pub struct InternalError(anyhow::Error);

impl IntoResponse for InternalError {
    fn into_response(self) -> Response {
        tracing::error!("{:?}", self.0);
        let error = ErrorTemplate;
        HtmlTemplate(error).into_response()
    }
}

impl<E> From<E> for InternalError
where
    E: Into<anyhow::Error>
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate;

pub async fn dashboard(
    session: Session,
    State(_state): State<Arc<InstancerState>>
) -> Response {
    if let Ok(Some(_uid)) = session.get::<String>("uid").await {
        let dashboard = DashboardTemplate;
        HtmlTemplate(dashboard).into_response()
    } else {
        Redirect::to("/login").into_response()
    }
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    oauth2_url: String,
    error: Option<&'static str>
}

pub async fn login(
    session: Session,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<InstancerState>>
) -> Result<Response, InternalError> {
    let (auth_url, _) = state.oauth2
        .authorize_url(CsrfToken::new_random)
        .add_scopes(discord::SCOPES.iter().map(|scope| Scope::new(scope.to_string())))
        .url();

    if let Some(code) = params.get("code") {
        match state.oauth2.exchange_code(AuthorizationCode::new(code.clone()))
                .request_async(async_http_client).await {
            Ok(token) => {
                let scopes = token.scopes().ok_or(anyhow::Error::msg("scopes are undefined"))?;
                let scopes: Vec<&str> = scopes.iter().map(|scope| scope.as_str()).collect();

                if !discord::SCOPES.iter().all(|sc1| scopes.iter().any(|sc2| sc1 == sc2)) {
                    let login = LoginTemplate { oauth2_url: auth_url.to_string(), error: Some("The OAuth2 token is missing one or more of the required scopes.") };
                    return Ok(HtmlTemplate(login).into_response());
                }

                let discord = Discord::new(token.access_token().secret().clone());
                let discord_user = discord.current_user().await?;

                let user = match state.database.fetch_user(&discord_user.id).await? {
                    None => {
                        let guilds = discord.current_guilds().await?;
                        if !guilds.iter().any(|guild| guild.id == state.config.discord.server_id) {
                            let login = LoginTemplate { oauth2_url: auth_url.to_string(), error: Some("You must be within the UnitedCTF Discord server.") };
                            return Ok(HtmlTemplate(login).into_response())
                        }

                        let new_user = User {
                            id: discord_user.id,
                            username: discord_user.username,
                            display_name: discord_user.global_name,
                            avatar: discord_user.avatar,
                            creation_time: TimeSinceEpoch::now().into()
                        };
                        state.database.insert_user(&new_user).await?;

                        new_user
                    }
                    Some(user) => user
                };

                session.insert("uid", user.id).await.unwrap();

                Ok(Redirect::to("/").into_response())
            },
            Err(_) => {
                let login = LoginTemplate { oauth2_url: auth_url.to_string(), error: Some("An invalid OAuth2 code was received from Discord.") };
                Ok(HtmlTemplate(login).into_response())
            }
        }
    } else {
        let login = LoginTemplate { oauth2_url: auth_url.to_string(), error: None };
        Ok(HtmlTemplate(login).into_response())
    }
}