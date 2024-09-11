use crate::config::InstancerConfig;
use crate::database::Database;
use crate::models::{ChallengeInstanceState, TimeSinceEpoch};
use serde::Serialize;
use std::cmp::{Ordering, PartialEq, Reverse};
use std::collections::{BinaryHeap, HashMap};
use std::path::PathBuf;
use std::process::{Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, Mutex};
use tokio::time;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub struct Challenge {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub ttl: u32,
    pub deployer_path: PathBuf
}

impl Challenge {
    pub async fn deploy(&self, user_id: &str, action: DeploymentRequestCommand) -> Result<String, ()> {
        let action_str = <DeploymentRequestCommand as Into<&str>>::into(action);

        tracing::debug!("[{}] calling script: \"{}\"", self.id, self.deployer_path.display());
        tracing::debug!("[{}] args: \"{}\" \"{}\" \"{}\"", self.id, action_str, &self.id, user_id);

        let mut command = Command::new(&self.deployer_path);
        command
            .arg(action_str)
            .arg(&self.id)
            .arg(user_id)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                tracing::error!("[{}] couldn't spawn child process: {:?}", self.id, err);
                return Err(());
            }
        };

        let (mut stdout, mut stderr) = match child.stdout.take().zip(child.stderr.take()) {
            None => {
                tracing::error!("[{}] couldn't take stdout & stderr", self.id);
                return Err(());
            },
            Some((stdout, stderr)) => (BufReader::new(stdout).lines(), BufReader::new(stderr).lines())
        };

        let mut details = String::new();

        loop {
            tokio::select! {
                Ok(Some(line)) = stdout.next_line() => {
                    tracing::debug!("[{}] [O] {}", self.id, line);
                    if line.starts_with("$") {
                        if details.len() != 0 { details.push('\n'); }
                        details.push_str(&line[2..]);
                    }
                }
                Ok(Some(line)) = stderr.next_line() => {
                    tracing::warn!("[{}] [E] {}", self.id, line);
                }
                else => break
            }
        }

        let output = child.wait_with_output().await.map_err(|_| ())?;
        if output.status.success() {
            Ok(details)
        } else {
            match output.status.code() {
                None => tracing::error!("[{}] child process exited with signal", self.id),
                Some(code) => tracing::error!("[{}] child process exited with status {}", self.id, code)
            }
            Err(())
        }
    }

    pub fn ttl_duration(&self) -> Duration {
        Duration::from_secs(self.ttl as u64)
    }
}

#[derive(Debug)]
pub struct DeploymentRequest {
    pub user_id: String,
    pub challenge_id: String,
    pub command: DeploymentRequestCommand
}

#[derive(Debug)]
pub enum DeploymentRequestCommand {
    Start,
    Stop,
    Restart,
    Cleanup
}

impl From<DeploymentRequestCommand> for &str {
    fn from(value: DeploymentRequestCommand) -> Self {
        match value {
            DeploymentRequestCommand::Start => "start",
            DeploymentRequestCommand::Stop => "stop",
            DeploymentRequestCommand::Restart => "restart",
            DeploymentRequestCommand::Cleanup => "cleanup"
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeploymentUpdate {
    pub user_id: String,
    pub challenge_id: String,
    pub details: DeploymentUpdateDetails
}

#[derive(Debug, Clone)]
pub enum DeploymentUpdateDetails {
    StateChange { state: ChallengeInstanceState, details: Option<String>, stop_time: Option<TimeSinceEpoch> },
    Message { contents: String, severity: MessageSeverity }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageSeverity {
    Success,
    Info,
    Warning,
    Error
}

#[derive(Eq)]
struct ChallengeInstanceOrdered {
    pub user_id: String,
    pub challenge_id: String,
    pub stop_time: TimeSinceEpoch
}

impl Ord for ChallengeInstanceOrdered {
    fn cmp(&self, other: &Self) -> Ordering {
        self.stop_time.cmp(&other.stop_time)
    }
}

impl PartialOrd for ChallengeInstanceOrdered {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.stop_time.cmp(&other.stop_time))
    }
}

impl PartialEq for ChallengeInstanceOrdered {
    fn eq(&self, other: &Self) -> bool {
        self.stop_time == other.stop_time
    }
}

pub struct DeploymentWorker {
    request_rx: async_channel::Receiver<DeploymentRequest>,
    pub request_tx: async_channel::Sender<DeploymentRequest>,
    pub update_tx: broadcast::Sender<DeploymentUpdate>,
    pub challenges: HashMap<String, Challenge>,
    pub database: Database,
    ttl_expiries: Mutex<BinaryHeap<Reverse<ChallengeInstanceOrdered>>>,
    shutdown_token: CancellationToken
}

impl DeploymentWorker {
    pub fn new(config: &InstancerConfig, database: Database, shutdown_token: CancellationToken) -> Self {
        let (request_tx, request_rx) = async_channel::unbounded();
        let (update_tx, _) = broadcast::channel(16);

        let challenges = config.challenges.iter()
            .filter_map(|(id, cfg)|
                config.deployers.get(&cfg.deployer).map(|deployer| {
                    let challenge = Challenge {
                        id: id.clone(),
                        name: cfg.name.clone(),
                        description: cfg.description.clone(),
                        ttl: cfg.ttl,
                        deployer_path: deployer.path.clone(),
                    };
                    (id.clone(), challenge)
                })
            )
            .filter(|(_, challenge)| {
                if challenge.deployer_path.exists() {
                    true
                } else {
                    tracing::warn!("disabled challenge {}: deployer does not exist at \"{}\"", challenge.id, challenge.deployer_path.display());
                    false
                }
            })
            .collect();

        DeploymentWorker {
            request_rx,
            request_tx,
            update_tx,
            challenges,
            database,
            ttl_expiries: Mutex::new(BinaryHeap::new()),
            shutdown_token,
        }
    }

    pub async fn do_work(&self) -> anyhow::Result<()> {
        let request_rx = self.request_rx.clone();

        while !self.shutdown_token.is_cancelled() || request_rx.len() > 0 {
            let time_until_next_expiry = {
                let mut ttl_expiries = self.ttl_expiries.lock().await;

                loop {
                    let Some(next_expired) = ttl_expiries.peek() else { break Duration::from_secs(60); };

                    if next_expired.0.stop_time > TimeSinceEpoch::now() {
                        break &next_expired.0.stop_time - &TimeSinceEpoch::now();
                    };

                    let next_expired = ttl_expiries.pop().unwrap();

                    if self.database.transition_challenge_instance_state(&next_expired.0.user_id, &next_expired.0.challenge_id, ChallengeInstanceState::Running, ChallengeInstanceState::QueuedStop).await? {
                        let request = DeploymentRequest {
                            user_id: next_expired.0.user_id.clone(),
                            challenge_id: next_expired.0.challenge_id.clone(),
                            command: DeploymentRequestCommand::Stop
                        };
                        self.request_tx.send(request).await?;

                        let state_change = DeploymentUpdate {
                            user_id: next_expired.0.user_id.clone(),
                            challenge_id: next_expired.0.challenge_id.clone(),
                            details: DeploymentUpdateDetails::StateChange { state: ChallengeInstanceState::QueuedStop, details: None, stop_time: None }
                        };
                        let _ = self.update_tx.send(state_change);
                    }
                }
            };

            tokio::select! {
                _ = self.shutdown_token.cancelled() => {},
                _ = time::sleep(time_until_next_expiry) => {},
                req = request_rx.recv() => {
                    if let Ok(request) = req {
                        self.handle_request(request).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_request(&self, request: DeploymentRequest) -> anyhow::Result<()> {
        let Some(challenge) = self.challenges.get(&request.challenge_id) else { return Ok(()) };

        let (state_change, message) = match &request.command {
            DeploymentRequestCommand::Start => {
                match challenge.deploy(&request.user_id, DeploymentRequestCommand::Start).await {
                    Ok(details) => {
                        tracing::info!("started challenge {} for user {}", challenge.id, request.user_id);

                        let stop_time = TimeSinceEpoch::from_now(challenge.ttl_duration());

                        self.push_ttl(request.user_id.clone(), request.challenge_id.clone(), stop_time.clone()).await;
                        self.database.populate_running_challenge_instance(&request.user_id, &request.challenge_id, &details, stop_time.clone()).await?;

                        (
                            DeploymentUpdateDetails::StateChange { state: ChallengeInstanceState::Running, details: Some(details), stop_time: Some(stop_time) },
                            DeploymentUpdateDetails::Message {
                                contents: format!("Le défi <strong>{}</strong> a été démarré!", challenge.name),
                                severity: MessageSeverity::Success
                            }
                        )
                    }
                    Err(_) => {
                        tracing::error!("couldn't start challenge {} for user {}", challenge.id, request.user_id);

                        let cleanup_request = DeploymentRequest {
                            user_id: request.user_id.clone(),
                            challenge_id: request.challenge_id.clone(),
                            command: DeploymentRequestCommand::Cleanup,
                        };
                        self.request_tx.send(cleanup_request).await?;

                        (
                            DeploymentUpdateDetails::StateChange { state: ChallengeInstanceState::QueuedStart, details: None, stop_time: None },
                            DeploymentUpdateDetails::Message {
                                contents: format!("Le défi <strong>{}</strong> n'a pas pu être démarré.<br>Contactez un administrateur si l'erreur persiste.", challenge.name),
                                severity: MessageSeverity::Error
                            }
                        )
                    }
                }
            }
            DeploymentRequestCommand::Stop => {
                match challenge.deploy(&request.user_id, DeploymentRequestCommand::Stop).await {
                    Ok(_) => {
                        tracing::info!("stopped challenge {} for user {}", challenge.id, request.user_id);

                        self.pop_ttl(&request.user_id, &request.challenge_id).await;
                        self.database.delete_challenge_instance(&request.user_id, &request.challenge_id).await?;

                        (
                            DeploymentUpdateDetails::StateChange { state: ChallengeInstanceState::Stopped, details: None, stop_time: None },
                            DeploymentUpdateDetails::Message {
                                contents: format!("Le défi <strong>{}</strong> a été arrêté.", challenge.name),
                                severity: MessageSeverity::Success
                            }
                        )
                    }
                    Err(_) => {
                        tracing::error!("couldn't stop challenge {} for user {}", challenge.id, request.user_id);

                        let cleanup_request = DeploymentRequest {
                            user_id: request.user_id.clone(),
                            challenge_id: request.challenge_id.clone(),
                            command: DeploymentRequestCommand::Cleanup,
                        };
                        self.request_tx.send(cleanup_request).await?;

                        (
                            DeploymentUpdateDetails::StateChange { state: ChallengeInstanceState::QueuedStop, details: None, stop_time: None },
                            DeploymentUpdateDetails::Message {
                                contents: format!("Le défi <strong>{}</strong> n'a pas pu être arrêté.<br>Contactez un administrateur si l'erreur persiste.", challenge.name),
                                severity: MessageSeverity::Error
                            }
                        )
                    }
                }
            }
            DeploymentRequestCommand::Restart => {
                match challenge.deploy(&request.user_id, DeploymentRequestCommand::Restart).await {
                    Ok(_) => {
                        tracing::info!("restarted challenge {} for user {}", challenge.id, request.user_id);

                        self.database.update_challenge_instance_state(&request.user_id, &request.challenge_id, ChallengeInstanceState::Running).await?;

                        (
                            DeploymentUpdateDetails::StateChange { state: ChallengeInstanceState::Running, details: None, stop_time: None },
                            DeploymentUpdateDetails::Message {
                                contents: format!("Le défi <strong>{}</strong> a été redémarré!", challenge.name),
                                severity: MessageSeverity::Success
                            }
                        )
                    }
                    Err(_) => {
                        tracing::error!("couldn't restart challenge {} for user {}", challenge.id, request.user_id);

                        let cleanup_request = DeploymentRequest {
                            user_id: request.user_id.clone(),
                            challenge_id: request.challenge_id.clone(),
                            command: DeploymentRequestCommand::Cleanup,
                        };
                        self.request_tx.send(cleanup_request).await?;

                        (
                            DeploymentUpdateDetails::StateChange { state: ChallengeInstanceState::QueuedRestart, details: None, stop_time: None },
                            DeploymentUpdateDetails::Message {
                                contents: format!("Le défi <strong>{}</strong> n'a pas pu être redémarré.<br>Contactez un administrateur si l'erreur persiste.", challenge.name),
                                severity: MessageSeverity::Error
                            }
                        )
                    }
                }
            }
            DeploymentRequestCommand::Cleanup => {
                match challenge.deploy(&request.user_id, DeploymentRequestCommand::Cleanup).await {
                    Ok(_) => {
                        tracing::info!("cleaned up challenge {} for user {}", challenge.id, request.user_id);

                        self.pop_ttl(&request.user_id, &request.challenge_id).await;
                        self.database.delete_challenge_instance(&request.user_id, &request.challenge_id).await?;

                        (
                            DeploymentUpdateDetails::StateChange { state: ChallengeInstanceState::Stopped, details: None, stop_time: None },
                            DeploymentUpdateDetails::Message {
                                contents: format!("Le défi <strong>{}</strong> a été réinitialisé.", challenge.name),
                                severity: MessageSeverity::Info
                            }
                        )
                    }
                    Err(_) => panic!("failed to clean up challenge {} for user {}", challenge.id, request.user_id)
                }
            }
        };

        let state_change = DeploymentUpdate {
            user_id: request.user_id.clone(),
            challenge_id: request.challenge_id.clone(),
            details: state_change,
        };
        let _ = self.update_tx.send(state_change);

        let message = DeploymentUpdate {
            user_id: request.user_id,
            challenge_id: request.challenge_id,
            details: message
        };
        let _ = self.update_tx.send(message);

        Ok(())
    }

    pub async fn prepare(&self) -> anyhow::Result<()> {
        let challenge_instances = self.database.get_challenge_instances().await?;

        for instance in challenge_instances.iter().filter(|instance| instance.state.is_queued()) {
            let cleanup_request = DeploymentRequest {
                user_id: instance.user_id.clone(),
                challenge_id: instance.challenge_id.clone(),
                command: DeploymentRequestCommand::Cleanup,
            };
            self.request_tx.send(cleanup_request).await?;
        }

        let mut ttl_expiries = self.ttl_expiries.lock().await;
        for instance in challenge_instances.into_iter().filter(|instance| instance.state == ChallengeInstanceState::Running) {
            ttl_expiries.push(Reverse(ChallengeInstanceOrdered {
                user_id: instance.user_id,
                challenge_id: instance.challenge_id,
                stop_time: instance.stop_time.unwrap()
            }));
        }

        Ok(())
    }

    pub async fn push_ttl(&self, user_id: String, challenge_id: String, stop_time: TimeSinceEpoch) {
        self.pop_ttl(&user_id, &challenge_id).await;

        let mut ttl_expiries = self.ttl_expiries.lock().await;
        ttl_expiries.push(Reverse(ChallengeInstanceOrdered {
            user_id,
            challenge_id,
            stop_time
        }));
    }

    pub async fn pop_ttl(&self, user_id: &str, challenge_id: &str) {
        let mut heap = self.ttl_expiries.lock().await;
        let mut buffer = Vec::with_capacity(heap.len());

        while let Some(val) = heap.pop() {
            if val.0.user_id == user_id && val.0.challenge_id == challenge_id { continue; }
            buffer.push(val);
        }

        for val in buffer.into_iter() {
            heap.push(val);
        }
    }
}