use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub watched_directories: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            watched_directories: Vec::new(),
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("invoices-renamer");
        fs::create_dir_all(&path).ok();
        path.push("config.json");
        path
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::config_path();
        if path.exists() {
            let contents = fs::read_to_string(path)?;
            let config: Config = serde_json::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(path, contents)?;
        Ok(())
    }

    pub fn add_directory(&mut self, dir: String) {
        if !self.watched_directories.contains(&dir) {
            self.watched_directories.push(dir);
        }
    }

    #[allow(dead_code)]
    pub fn remove_directory(&mut self, dir: &str) {
        self.watched_directories.retain(|d| d != dir);
    }
}
