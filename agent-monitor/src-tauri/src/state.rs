use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub pid: u32,
    pub exe_path: String,
    pub working_dir: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub polling_interval_ms: u64,
    pub click_action: String,
    pub auto_start: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            polling_interval_ms: 2000,
            click_action: "terminal".to_string(),
            auto_start: false,
        }
    }
}

pub struct AppState {
    pub agents: Mutex<HashMap<u32, AgentInfo>>,
    pub previous_pids: Mutex<HashSet<u32>>,
    pub config: Mutex<Config>,
}

impl AppState {
    pub fn new() -> Self {
        let cfg = crate::config::load_config().unwrap_or_default();
        Self {
            agents: Mutex::new(HashMap::new()),
            previous_pids: Mutex::new(HashSet::new()),
            config: Mutex::new(cfg),
        }
    }
}
