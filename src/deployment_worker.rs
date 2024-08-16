use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::sync::mpsc;
use tokio::time;
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
            // artificial "processing" time
            time::sleep(Duration::from_secs(1)).await;

            let new_state = match &request.command {
                DeploymentRequestCommand::Start => {
                    // starting work here

                    self.database.update_challenge_instance_state(
                        &request.user_id, &request.challenge_id,
                        ChallengeInstanceState::Running
                    ).await?;
                    ChallengeInstanceState::Running
                }
                DeploymentRequestCommand::Stop => {
                    // stopping work here

                    self.database.stop_challenge_instance(&request.user_id, &request.challenge_id).await?;
                    ChallengeInstanceState::Stopped
                }
                DeploymentRequestCommand::Restart => {
                    // restarting work here

                    self.database.update_challenge_instance_state(
                        &request.user_id, &request.challenge_id,
                        ChallengeInstanceState::Running
                    ).await?;
                    ChallengeInstanceState::Running
                }
            };

            let deployment_state_change = DeploymentUpdate {
                user_id: request.user_id,
                challenge_id: request.challenge_id,
                details: DeploymentUpdateDetails::StateChange { state: new_state },
            };
            let _ = self.update_tx.write().await.send(deployment_state_change);
        }

        Ok(())
    }
}