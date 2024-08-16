use std::collections::HashMap;
use std::path::PathBuf;

use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::sync::mpsc;

use crate::config::InstancerConfig;

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
    pub challenge_id: String
}

#[derive(Clone)]
pub struct DeploymentUpdate {
    pub user_id: String,
    pub challenge_id: String
}

pub struct DeploymentWorker {
    request_rx: Mutex<mpsc::Receiver<DeploymentRequest>>,
    pub request_tx: mpsc::Sender<DeploymentRequest>,
    pub update_tx: RwLock<broadcast::Sender<DeploymentUpdate>>,
    pub challenges: HashMap<String, Challenge>
}

impl DeploymentWorker {
    pub fn new(config: &InstancerConfig) -> Self {
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
            challenges
        }
    }

    pub async fn do_work(&self) {
        let mut request_rx = self.request_rx.lock().await;
        while let Some(request) = request_rx.recv().await {
            println!("{:#?}", request);
        }
    }
}