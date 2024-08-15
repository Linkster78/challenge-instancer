use std::collections::HashMap;
use std::path::PathBuf;

use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task;

use crate::config::{DeployerConfig, InstancerConfig};

#[derive(Debug)]
pub struct Challenge {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub ttl: u32,
    pub deployer_path: PathBuf
}

pub struct DeploymentRequest {
    pub user_id: String,
    pub challenge_id: String
}

#[derive(Clone)]
pub struct DeploymentUpdate {
    user_id: String,
    challenge_id: String,
    state: DeploymentState
}

#[derive(Clone)]
pub enum DeploymentState {
    None,
    Queued,
    Deploying,
    Deployed,
    Failed
}

pub struct DeploymentWorker {
    pub request_tx: mpsc::Sender<DeploymentRequest>,
    pub update_rx: broadcast::Receiver<DeploymentUpdate>,
    pub challenges: HashMap<String, Challenge>
}

impl DeploymentWorker {
    pub fn new(config: &InstancerConfig) -> Self {
        let (request_tx, mut request_rx) = mpsc::channel(128);
        let (update_tx, update_rx) = broadcast::channel(16);

        task::spawn(async move { Self::do_work(request_rx, update_tx).await });

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

        println!("{:#?}", challenges);

        DeploymentWorker {
            request_tx,
            update_rx,
            challenges
        }
    }

    async fn do_work(mut request_rx: mpsc::Receiver<DeploymentRequest>, update_tx: broadcast::Sender<DeploymentUpdate>) {
        while let Some(request) = request_rx.recv().await {

        }
    }
}