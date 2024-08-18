use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use crate::config::InstancerConfig;
use crate::database::Database;
use crate::models::ChallengeInstanceState;

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

        let mut command = Command::new("/bin/bash");
        command
            .arg(&self.deployer_path)
            .arg(action_str)
            .arg(&self.id)
            .arg(user_id)
            .arg("2>&1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                tracing::error!("[{}] couldn't spawn child process: {:?}", self.id, err);
                return Err(());
            }
        };

        let stdout = match &mut child.stdout {
            None => return Err(()),
            Some(stdout) => BufReader::new(stdout)
        };
        let mut stdout_lines = stdout.lines();

        let mut details = String::new();
        while let Some(line) = stdout_lines.next_line().await.map_err(|_| ())? {
            tracing::debug!("[{}] {}", self.id, line);
            if line.starts_with("$") {
                if details.len() != 0 { details.push('\n'); }
                details.push_str(&line[2..]);
            }
        }

        let output = child.wait_with_output().await.map_err(|_| ())?;
        if output.status.success() {
            Ok(details)
        } else {
            Err(())
        }
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
    Restart
}

impl From<DeploymentRequestCommand> for &str {
    fn from(value: DeploymentRequestCommand) -> Self {
        match value {
            DeploymentRequestCommand::Start => "start",
            DeploymentRequestCommand::Stop => "stop",
            DeploymentRequestCommand::Restart => "restart"
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
    StateChange { state: ChallengeInstanceState }
}

pub struct DeploymentWorker {
    request_rx: Mutex<mpsc::Receiver<DeploymentRequest>>,
    pub request_tx: mpsc::Sender<DeploymentRequest>,
    pub update_tx: RwLock<broadcast::Sender<DeploymentUpdate>>,
    pub challenges: HashMap<String, Challenge>,
    pub database: Database,
    shutdown_token: CancellationToken
}

impl DeploymentWorker {
    pub fn new(config: &InstancerConfig, database: Database, shutdown_token: CancellationToken) -> Self {
        let (request_tx, request_rx) = mpsc::channel(128);
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
            request_rx: Mutex::new(request_rx),
            request_tx,
            update_tx: RwLock::new(update_tx),
            challenges,
            database,
            shutdown_token,
        }
    }

    pub async fn do_work(&self) -> anyhow::Result<()> {
        let mut request_rx = self.request_rx.lock().await;

        while !self.shutdown_token.is_cancelled() || request_rx.len() > 0 {
            tokio::select! {
                _ = self.shutdown_token.cancelled() => {},
                req = request_rx.recv() => {
                    if let Some(request) = req {
                        self.handle_request(request).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_request(&self, request: DeploymentRequest) -> anyhow::Result<()> {
        let Some(challenge) = self.challenges.get(&request.challenge_id) else { return Ok(()) };

        let new_state = match &request.command {
            DeploymentRequestCommand::Start => {
                match challenge.deploy(&request.user_id, DeploymentRequestCommand::Start).await {
                    Ok(details) => {
                        tracing::info!("started challenge {} for user {}", challenge.id, request.user_id);

                        self.database.populate_running_challenge_instance(&request.user_id, &request.challenge_id, &details).await?;
                        ChallengeInstanceState::Running
                    }
                    Err(_) => {
                        tracing::error!("couldn't start challenge {} for user {}", challenge.id, request.user_id);

                        self.database.delete_challenge_instance(&request.user_id, &request.challenge_id).await?;
                        ChallengeInstanceState::Stopped
                    }
                }
            }
            DeploymentRequestCommand::Stop => {
                match challenge.deploy(&request.user_id, DeploymentRequestCommand::Stop).await {
                    Ok(_) => {
                        tracing::info!("stopped challenge {} for user {}", challenge.id, request.user_id);

                        self.database.delete_challenge_instance(&request.user_id, &request.challenge_id).await?;
                        ChallengeInstanceState::Stopped
                    }
                    Err(_) => {
                        tracing::error!("couldn't stop challenge {} for user {}", challenge.id, request.user_id);

                        self.database.update_challenge_instance_state(&request.user_id, &request.challenge_id, ChallengeInstanceState::Running).await?;
                        ChallengeInstanceState::Running
                    }
                }
            }
            DeploymentRequestCommand::Restart => {
                match challenge.deploy(&request.user_id, DeploymentRequestCommand::Restart).await {
                    Ok(_) => {
                        tracing::info!("restarted challenge {} for user {}", challenge.id, request.user_id);

                        self.database.update_challenge_instance_state(&request.user_id, &request.challenge_id, ChallengeInstanceState::Running).await?;
                        ChallengeInstanceState::Running
                    }
                    Err(_) => {
                        tracing::error!("couldn't restart challenge {} for user {}", challenge.id, request.user_id);

                        self.database.update_challenge_instance_state(&request.user_id, &request.challenge_id, ChallengeInstanceState::Running).await?;
                        ChallengeInstanceState::Running
                    }
                }
            }
        };

        let deployment_state_change = DeploymentUpdate {
            user_id: request.user_id,
            challenge_id: request.challenge_id,
            details: DeploymentUpdateDetails::StateChange { state: new_state },
        };
        let _ = self.update_tx.write().await.send(deployment_state_change);

        Ok(())
    }
}