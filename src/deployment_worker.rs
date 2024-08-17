use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::sync::mpsc;
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

        let mut command = Command::new(&self.deployer_path);
        command.arg(action_str);
        command.arg(&self.id);
        command.arg(user_id);

        let (success, output): (bool, String) = match command.output().await {
            Ok(output) => (output.status.success(), String::from_utf8_lossy(&output.stdout).to_string()),
            Err(err) => {
                tracing::error!("[{}] couldn't spawn child process: {:?}", self.id, err);
                return Err(());
            }
        };

        let mut details = String::new();
        for line in output.lines() {
            tracing::debug!("[{}] {}", self.id, line);
            if success && line.starts_with("$") {
                if details.len() != 0 { details.push('\n'); }
                details.push_str(&line[2..]);
            }
        }

        if success {
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
    pub database: Database
}

impl DeploymentWorker {
    pub fn new(config: &InstancerConfig, database: Database) -> Self {
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
            .collect();

        DeploymentWorker {
            request_rx: Mutex::new(request_rx),
            request_tx,
            update_tx: RwLock::new(update_tx),
            challenges,
            database
        }
    }

    pub async fn do_work(&self) -> anyhow::Result<()> {
        let mut request_rx = self.request_rx.lock().await;

        while let Some(request) = request_rx.recv().await {
            if let Some(challenge) = self.challenges.get(&request.challenge_id) {
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
            }
        }

        Ok(())
    }
}