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
use crate::models::{Challenge, TimeSinceEpoch, User};
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
struct DashboardTemplate {
    avatar_url: String,
    challenges: Vec<Challenge>
}

pub async fn dashboard(
    session: Session,
    State(_state): State<Arc<InstancerState>>
) -> Response {
    if let Ok(Some(uid)) = session.get::<String>("uid").await {
        let dashboard = DashboardTemplate {
            avatar_url: Discord::avatar_url(&uid, &session.get::<String>("avatar").await.unwrap().unwrap()),
            challenges: Vec::new()
        };
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
        .add_extra_param("prompt", "none")
        .url();

    if let Some(code) = params.get("code") {
        match state.oauth2.exchange_code(AuthorizationCode::new(code.clone()))
                .request_async(async_http_client).await {
            Ok(token) => {
                let scopes = token.scopes().ok_or(anyhow::Error::msg("scopes are undefined"))?;
                let scopes: Vec<&str> = scopes.iter().map(|scope| scope.as_str()).collect();

                if !discord::SCOPES.iter().all(|sc1| scopes.iter().any(|sc2| sc1 == sc2)) {
                    let login = LoginTemplate { oauth2_url: auth_url.to_string(), error: Some("Certains des scopes OAuth requis n'ont pas été autorisés.") };
                    return Ok(HtmlTemplate(login).into_response());
                }

                let discord = Discord::new(token.access_token().secret().clone());
                let discord_user = discord.current_user().await?;

                let user = match state.database.fetch_user(&discord_user.id).await? {
                    None => {
                        let guilds = discord.current_guilds().await?;
                        if !guilds.iter().any(|guild| guild.id == state.config.discord.server_id) {
                            let login = LoginTemplate { oauth2_url: auth_url.to_string(), error: Some("Vous devez faire partie du serveur Discord du UnitedCTF pour utiliser cette plateforme.") };
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
                session.insert("avatar", user.avatar).await.unwrap();

                Ok(Redirect::to("/").into_response())
            },
            Err(_) => {
                let login = LoginTemplate { oauth2_url: auth_url.to_string(), error: Some("Un code OAuth invalide a été reçu de la part de Discord.") };
                Ok(HtmlTemplate(login).into_response())
            }
        }
    } else {
        let login = LoginTemplate { oauth2_url: auth_url.to_string(), error: None };
        Ok(HtmlTemplate(login).into_response())
    }
}