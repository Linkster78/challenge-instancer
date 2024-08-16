use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use askama::Template;
use axum::Error;
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::extract::ws::{Message, WebSocket};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use futures::FutureExt;
use oauth2::{AuthorizationCode, CsrfToken, Scope, TokenResponse};
use oauth2::reqwest::async_http_client;
use serde::{Deserialize, Serialize};
use tokio::time;
use tower_sessions::{Session, SessionStore};
use tower_sessions::session::Id;

use crate::{discord, InstancerState};
use crate::deployment_worker::{DeploymentRequest};
use crate::discord::Discord;
use crate::models::{Challenge, ChallengeInstance, ChallengeInstanceState, TimeSinceEpoch, User};
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
    avatar_url: String
}

pub async fn dashboard(
    session: Session,
    State(_state): State<Arc<InstancerState>>
) -> Result<Response, InternalError> {
    if let Some(uid) = session.get::<String>("uid").await? {
        let dashboard = DashboardTemplate {
            avatar_url: Discord::avatar_url(&uid, &session.get::<String>("avatar").await.unwrap().unwrap())
        };
        Ok(HtmlTemplate(dashboard).into_response())
    } else {
        Ok(Redirect::to("/login").into_response())
    }
}

#[derive(Serialize, Debug)]
struct ChallengePlayerState {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub ttl: u32,
    pub state: ChallengeInstanceState
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerBoundMessage {
    ChallengeStart { id: String },
    ChallengeStop { id: String },
    ChallengeRestart { id: String },
    ChallengeExtend { id: String }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientBoundMessage {
    ChallengeListing { challenges: HashMap<String, ChallengePlayerState> },
    ChallengeStateChange { id: String, state: ChallengeInstanceState }
}

impl From<ClientBoundMessage> for Message {
    fn from(value: ClientBoundMessage) -> Self {
        Message::Text(serde_json::to_string(&value).unwrap())
    }
}

impl TryFrom<Message> for ServerBoundMessage {
    type Error = anyhow::Error;
    fn try_from(value: Message) -> Result<Self, Self::Error> {
        if let Message::Text(text) = value {
            Ok(serde_json::from_str(&text)?)
        } else {
            Err(anyhow!("invalid message variant, only Text is supported"))
        }
    }
}

pub async fn dashboard_ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<InstancerState>>
) -> Result<Response, InternalError> {
    let Some(session_id) = params.get("sid").and_then(|sid: &String| Id::from_str(sid.as_str()).ok()) else {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    };

    let Some(session) = state.session_store.load(&session_id).await? else {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    };

    let Some(uid): Option<String> = session.data.get("uid").and_then(|val| val.as_str()).map(|s| s.to_string()) else {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    };

    Ok(ws.on_upgrade(move |socket| dashboard_handle_ws(Arc::clone(&state), socket, uid)))
}

pub async fn dashboard_handle_ws(state: Arc<InstancerState>, mut socket: WebSocket, uid: String) {
    let mut request_tx = state.deployer.request_tx.clone();
    let mut update_rx = state.deployer.update_tx.read().await.subscribe();

    let challenges: HashMap<String, ChallengePlayerState> = state.deployer.challenges.iter()
        .map(|(id, challenge)| {
            let challenge = ChallengePlayerState {
                id: challenge.id.clone(),
                name: challenge.name.clone(),
                description: challenge.description.clone(),
                ttl: challenge.ttl,
                state: ChallengeInstanceState::Stopped
            };
            (id.clone(), challenge)
        })
        .collect();

    let challenge_listing = ClientBoundMessage::ChallengeListing { challenges };
    let _ = socket.send(challenge_listing.into()).await;

    loop {
        if let Some(msg) = socket.recv().now_or_never() {
            match msg.map(|res| res.ok().and_then(|msg| ServerBoundMessage::try_from(msg).ok())) {
                Some(Some(msg)) => match msg {
                    ServerBoundMessage::ChallengeStart { id: cid } => {
                        let instance = ChallengeInstance {
                            user_id: uid.clone(),
                            challenge_id: cid.clone(),
                            state: ChallengeInstanceState::QueuedStart,
                            start_time: TimeSinceEpoch::now()
                        };

                        if let Ok(()) = state.database.insert_challenge_instance(&instance).await {
                            let request = DeploymentRequest {
                                user_id: uid.clone(),
                                challenge_id: cid.clone()
                            };
                            request_tx.send(request).await.unwrap();

                            let challenge_state_change = ClientBoundMessage::ChallengeStateChange { id: cid, state: ChallengeInstanceState::QueuedStart };
                            let _ = socket.send(challenge_state_change.into()).await;
                        }
                    }
                    ServerBoundMessage::ChallengeStop { id: cid } => {

                    }
                    ServerBoundMessage::ChallengeRestart { id: cid } => {

                    }
                    ServerBoundMessage::ChallengeExtend { id: cid } => {

                    }
                }
                _ => break
            }
        }

        while let Ok(update) = update_rx.try_recv() {
            if update.user_id != uid { continue; }
            // handle update
        }

        // Stop the CPU from getting murdered
        time::sleep(Duration::from_millis(5)).await;
    }
}

pub async fn logout(
    session: Session
) -> impl IntoResponse {
    session.clear().await;
    Redirect::to("/login")
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
) -> Result<impl IntoResponse, InternalError> {
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
                            creation_time: TimeSinceEpoch::now()
                        };
                        // We can ignore the error here, this could only fail in the case of
                        // a race condition, which wouldn't influence the rest of the function
                        let _ = state.database.insert_user(&new_user).await;

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
