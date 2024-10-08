use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use anyhow::anyhow;
use askama::Template;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use governor::clock::{Clock, QuantaClock};
use oauth2::reqwest::async_http_client;
use oauth2::{AuthorizationCode, CsrfToken, Scope, TokenResponse};
use serde::{Deserialize, Serialize};
use tower_sessions::session::Id;
use tower_sessions::{Session, SessionStore};

use crate::deployment_worker::{DeploymentRequest, DeploymentRequestCommand, DeploymentUpdateDetails, MessageSeverity};
use crate::discord::Discord;
use crate::models::{ChallengeInstance, ChallengeInstanceState, TimeSinceEpoch, User};
use crate::templating::HtmlTemplate;
use crate::{discord, InstancerState};
use crate::database::ChallengeInstanceInsertionResult;

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
            avatar_url: Discord::avatar_url(&uid, &session.get::<Option<String>>("avatar").await?.unwrap())
        };
        Ok(HtmlTemplate(dashboard).into_response())
    } else {
        Ok(Redirect::to("/login").into_response())
    }
}

#[derive(Template)]
#[template(path = "help.html")]
struct HelpTemplate {
    avatar_url: String
}

pub async fn help(
    session: Session,
    State(_state): State<Arc<InstancerState>>
) -> Result<Response, InternalError> {
    if let Some(uid) = session.get::<String>("uid").await? {
        let help = HelpTemplate {
            avatar_url: Discord::avatar_url(&uid, &session.get::<Option<String>>("avatar").await?.unwrap())
        };
        Ok(HtmlTemplate(help).into_response())
    } else {
        Ok(Redirect::to("/login").into_response())
    }
}

#[derive(Serialize, Debug)]
pub struct ChallengePlayerState {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub state: ChallengeInstanceState,
    pub stop_time: Option<TimeSinceEpoch>,
    pub details: Option<String>
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerBoundMessage {
    ChallengeAction { id: String, action: ChallengeActionCommand },
    Heartbeat
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ChallengeActionCommand {
    Start,
    Stop,
    Restart,
    Extend
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientBoundMessage {
    ChallengeListing { challenges: HashMap<String, ChallengePlayerState> },
    ChallengeStateChange { id: String, state: ChallengeInstanceState, details: Option<String>, stop_time: Option<TimeSinceEpoch> },
    Message { id: String, contents: String, severity: MessageSeverity },
    Heartbeat
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

    Ok(ws.on_upgrade(move |socket| dashboard_handle_ws_unwrap(Arc::clone(&state), socket, uid)))
}

pub async fn dashboard_handle_ws_unwrap(state: Arc<InstancerState>, socket: WebSocket, uid: String) {
    dashboard_handle_ws(state, socket, uid).await.unwrap()
}

pub async fn dashboard_handle_ws(state: Arc<InstancerState>, mut socket: WebSocket, uid: String) -> anyhow::Result<()> {
    let request_tx = state.deployer.request_tx.clone();
    let mut update_rx = state.deployer.update_tx.subscribe();

    let challenge_instances = state.database.get_user_challenge_instances(&uid).await?;
    let challenges: HashMap<String, ChallengePlayerState> = state.deployer.challenges.iter()
        .map(|(id, challenge)| {
            let (state, stop_time, details) = match challenge_instances.iter().filter(|instance| &instance.challenge_id == id).next() {
                None => (ChallengeInstanceState::Stopped, None, None),
                Some(instance) => (instance.state.clone(), instance.stop_time.clone(), instance.details.clone())
            };

            let challenge = ChallengePlayerState {
                id: challenge.id.clone(),
                name: challenge.name.clone(),
                description: challenge.description.clone(),
                stop_time,
                state,
                details
            };

            (id.clone(), challenge)
        })
        .collect();

    let challenge_listing = ClientBoundMessage::ChallengeListing { challenges };
    let _ = socket.send(challenge_listing.into()).await;

    loop {
        tokio::select! {
            Some(res) = socket.recv() => {
                if state.shutdown_token.is_cancelled() { continue; }

                match res.ok().and_then(|m| ServerBoundMessage::try_from(m).ok()) {
                    Some(msg) => match msg {
                        ServerBoundMessage::ChallengeAction { id: cid, action } => match state.deployer.challenges.get(&cid) {
                            Some(challenge) => {
                                if let Err(not_until) = state.rate_limiter.check_key(&uid) {
                                    let clock = QuantaClock::default();
                                    let duration_until = not_until.wait_time_from(clock.now());

                                    let seconds_until = duration_until.as_secs_f32().ceil();

                                    let message = ClientBoundMessage::Message {
                                        id: challenge.id.clone(),
                                        severity: MessageSeverity::Warning,
                                        contents: format!("Veuillez attendre {} seconde{} avant votre prochaine action.", seconds_until, if seconds_until == 1.0 { "" } else { "s" }),
                                    };
                                    let _ = socket.send(message.into()).await;
                                    continue;
                                }

                                match action {
                                    ChallengeActionCommand::Start => {
                                        let instance = ChallengeInstance {
                                            user_id: uid.clone(),
                                            challenge_id: cid.clone(),
                                            state: ChallengeInstanceState::QueuedStart,
                                            stop_time: None,
                                            details: None
                                        };

                                        match state.database.insert_challenge_instance(&instance, state.config.settings.max_concurrent_challenges).await? {
                                            ChallengeInstanceInsertionResult::Inserted => {
                                                let request = DeploymentRequest {
                                                    user_id: uid.clone(),
                                                    challenge_id: cid.clone(),
                                                    command: DeploymentRequestCommand::Start
                                                };
                                                request_tx.send(request).await?;

                                                let challenge_state_change = ClientBoundMessage::ChallengeStateChange { id: cid, state: ChallengeInstanceState::QueuedStart, details: None, stop_time: None};
                                                let _ = socket.send(challenge_state_change.into()).await;
                                            }
                                            ChallengeInstanceInsertionResult::LimitReached => {
                                                let message = ClientBoundMessage::Message {
                                                    id: cid,
                                                    severity: MessageSeverity::Warning,
                                                    contents: format!("Vous avez atteint la limite de {} défis concurrents.", state.config.settings.max_concurrent_challenges),
                                                };
                                                let _ = socket.send(message.into()).await;
                                            }
                                            ChallengeInstanceInsertionResult::Exists => {}
                                        }
                                    }
                                    ChallengeActionCommand::Stop => {
                                        if state.database.transition_challenge_instance_state(&uid, &cid, ChallengeInstanceState::Running, ChallengeInstanceState::QueuedStop).await? {
                                            let request = DeploymentRequest {
                                                user_id: uid.clone(),
                                                challenge_id: cid.clone(),
                                                command: DeploymentRequestCommand::Stop
                                            };
                                            request_tx.send(request).await?;

                                            let challenge_state_change = ClientBoundMessage::ChallengeStateChange { id: cid, state: ChallengeInstanceState::QueuedStop, details: None, stop_time: None};
                                            let _ = socket.send(challenge_state_change.into()).await;
                                        }
                                    }
                                    ChallengeActionCommand::Restart => {
                                        if state.database.transition_challenge_instance_state(&uid, &cid, ChallengeInstanceState::Running, ChallengeInstanceState::QueuedRestart).await? {
                                            let request = DeploymentRequest {
                                                user_id: uid.clone(),
                                                challenge_id: cid.clone(),
                                                command: DeploymentRequestCommand::Restart
                                            };
                                            request_tx.send(request).await?;

                                            let challenge_state_change = ClientBoundMessage::ChallengeStateChange { id: cid, state: ChallengeInstanceState::QueuedRestart, details: None, stop_time: None};
                                            let _ = socket.send(challenge_state_change.into()).await;
                                        }
                                    }
                                    ChallengeActionCommand::Extend => {
                                        let stop_time = TimeSinceEpoch::from_now(challenge.ttl_duration());

                                        if state.database.extend_challenge_instance(&uid, &cid, stop_time.clone()).await? {
                                            state.deployer.push_ttl(uid.clone(), cid.clone(), stop_time.clone()).await;

                                            let challenge_state_change = ClientBoundMessage::ChallengeStateChange { id: cid.clone(), state: ChallengeInstanceState::Running, details: None, stop_time: Some(stop_time) };
                                            let _ = socket.send(challenge_state_change.into()).await;

                                            let message = ClientBoundMessage::Message {
                                                id: cid,
                                                severity: MessageSeverity::Success,
                                                contents: format!("Le défi <strong>{}</strong> a été étendu.", challenge.name),
                                            };
                                            let _ = socket.send(message.into()).await;
                                        }
                                    }
                                }
                            }
                            None => return Ok(()) /* received command for unknown challenge from client, close connection */
                        },
                        ServerBoundMessage::Heartbeat => {
                            let _ = socket.send(ClientBoundMessage::Heartbeat.into()).await;
                        }
                    },
                    None => return Ok(()) /* received invalid message, close connection */
                }
            }
            Ok(update) = update_rx.recv() => {
                if update.user_id != uid { continue; }

                match update.details {
                    DeploymentUpdateDetails::StateChange { state, details, stop_time } => {
                        let challenge_state_change = ClientBoundMessage::ChallengeStateChange { id: update.challenge_id, state, details, stop_time };
                        let _ = socket.send(challenge_state_change.into()).await;
                    }
                    DeploymentUpdateDetails::Message { contents, severity } => {
                        let message = ClientBoundMessage::Message { id: update.challenge_id, contents, severity };
                        let _ = socket.send(message.into()).await;
                    }
                }
            },
            else => return Ok(()) /* socket has closed or update sender has closed, indicating that the deployment worker is down */
        }
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
                            username: discord_user.username.clone(),
                            display_name: discord_user.global_name.unwrap_or(discord_user.username),
                            avatar: discord_user.avatar,
                            creation_time: TimeSinceEpoch::now(),
                            instance_count: 0
                        };

                        state.database.insert_user(&new_user).await?;

                        new_user
                    }
                    Some(user) => user
                };

                session.insert("uid", user.id).await?;
                session.insert("avatar", user.avatar).await?;

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
