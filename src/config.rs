use crate::error::{ConfigError, ConfigResult};
use crate::models::BotConfig;
use std::env;

/// Configuration manager trait for loading and managing bot configuration
pub trait ConfigManager {
    /// Load configuration from environment variables
    fn load_config() -> ConfigResult<BotConfig>;
    /// Update the target channel ID
    fn update_target_channel(&mut self, channel_id: u64) -> ConfigResult<()>;
    /// Validate the current configuration
    fn validate_config(&self) -> ConfigResult<()>;
}

/// Default configuration manager implementation
pub struct DefaultConfigManager {
    config: BotConfig,
}

impl DefaultConfigManager {
    /// Create a new configuration manager
    pub fn new() -> ConfigResult<Self> {
        let config = Self::load_config()?;
        Ok(Self { config })
    }

    /// Get the current configuration
    pub fn config(&self) -> &BotConfig {
        &self.config
    }

    /// Get a mutable reference to the configuration
    pub fn config_mut(&mut self) -> &mut BotConfig {
        &mut self.config
    }
}

impl ConfigManager for DefaultConfigManager {
    fn load_config() -> ConfigResult<BotConfig> {
        let discord_token = env::var("DISCORD_TOKEN")
            .map_err(|_| ConfigError::MissingEnvironmentVariable {
                var_name: "DISCORD_TOKEN".to_string(),
            })?;

        let spotify_client_id = env::var("SPOTIFY_CLIENT_ID")
            .map_err(|_| ConfigError::MissingEnvironmentVariable {
                var_name: "SPOTIFY_CLIENT_ID".to_string(),
            })?;

        let spotify_client_secret = env::var("SPOTIFY_CLIENT_SECRET")
            .map_err(|_| ConfigError::MissingEnvironmentVariable {
                var_name: "SPOTIFY_CLIENT_SECRET".to_string(),
            })?;

        let spotify_refresh_token = env::var("SPOTIFY_REFRESH_TOKEN")
            .map_err(|_| ConfigError::MissingEnvironmentVariable {
                var_name: "SPOTIFY_REFRESH_TOKEN".to_string(),
            })?;

        let target_channel_id = env::var("TARGET_CHANNEL_ID")
            .map_err(|_| ConfigError::MissingEnvironmentVariable {
                var_name: "TARGET_CHANNEL_ID".to_string(),
            })?
            .parse::<u64>()
            .map_err(|_| ConfigError::InvalidValue {
                field: "TARGET_CHANNEL_ID".to_string(),
                value: env::var("TARGET_CHANNEL_ID").unwrap_or_default(),
            })?;

        let collaborative_playlist_id = env::var("COLLABORATIVE_PLAYLIST_ID")
            .map_err(|_| ConfigError::MissingEnvironmentVariable {
                var_name: "COLLABORATIVE_PLAYLIST_ID".to_string(),
            })?;

        let discovery_playlist_id = env::var("DISCOVERY_PLAYLIST_ID")
            .map_err(|_| ConfigError::MissingEnvironmentVariable {
                var_name: "DISCOVERY_PLAYLIST_ID".to_string(),
            })?;

        // Optional configuration with defaults
        let weekly_schedule_cron = env::var("WEEKLY_SCHEDULE_CRON")
            .unwrap_or_else(|_| "0 0 12 * * MON".to_string()); // Every Monday at noon

        let max_retry_attempts = env::var("MAX_RETRY_ATTEMPTS")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<u32>()
            .map_err(|_| ConfigError::InvalidValue {
                field: "MAX_RETRY_ATTEMPTS".to_string(),
                value: env::var("MAX_RETRY_ATTEMPTS").unwrap_or_default(),
            })?;

        let retry_base_delay_ms = env::var("RETRY_BASE_DELAY_MS")
            .unwrap_or_else(|_| "1000".to_string())
            .parse::<u64>()
            .map_err(|_| ConfigError::InvalidValue {
                field: "RETRY_BASE_DELAY_MS".to_string(),
                value: env::var("RETRY_BASE_DELAY_MS").unwrap_or_default(),
            })?;

        let retry_max_delay_ms = env::var("RETRY_MAX_DELAY_MS")
            .unwrap_or_else(|_| "30000".to_string())
            .parse::<u64>()
            .map_err(|_| ConfigError::InvalidValue {
                field: "RETRY_MAX_DELAY_MS".to_string(),
                value: env::var("RETRY_MAX_DELAY_MS").unwrap_or_default(),
            })?;

        let config = BotConfig {
            discord_token,
            spotify_client_id,
            spotify_client_secret,
            spotify_refresh_token,
            target_channel_id,
            collaborative_playlist_id,
            discovery_playlist_id,
            weekly_schedule_cron,
            max_retry_attempts,
            retry_base_delay_ms,
            retry_max_delay_ms,
        };

        // Validate the configuration
        config.validate().map_err(|msg| ConfigError::ValidationFailed(msg))?;

        Ok(config)
    }

    fn update_target_channel(&mut self, channel_id: u64) -> ConfigResult<()> {
        if channel_id == 0 {
            return Err(ConfigError::InvalidValue {
                field: "target_channel_id".to_string(),
                value: channel_id.to_string(),
            });
        }

        self.config.target_channel_id = channel_id;
        Ok(())
    }

    fn validate_config(&self) -> ConfigResult<()> {
        self.config.validate().map_err(|msg| ConfigError::ValidationFailed(msg))
    }
}

impl Default for DefaultConfigManager {
    fn default() -> Self {
        Self::new().expect("Failed to load configuration")
    }
}

/// Utility functions for configuration management
pub mod utils {
    use super::*;

    /// Load configuration with detailed error reporting
    pub fn load_config_with_details() -> ConfigResult<BotConfig> {
        // Load .env file if it exists
        let _ = dotenv::dotenv();
        
        match DefaultConfigManager::load_config() {
            Ok(config) => {
                log::info!("Configuration loaded successfully");
                log::debug!("Target channel ID: {}", config.target_channel_id);
                log::debug!("Collaborative playlist ID: {}", config.collaborative_playlist_id);
                log::debug!("Discovery playlist ID: {}", config.discovery_playlist_id);
                log::debug!("Weekly schedule: {}", config.weekly_schedule_cron);
                Ok(config)
            }
            Err(e) => {
                log::error!("Failed to load configuration: {:?}", e);
                match &e {
                    ConfigError::MissingEnvironmentVariable { var_name } => {
                        log::error!("Please set the {} environment variable", var_name);
                    }
                    ConfigError::InvalidValue { field, value } => {
                        log::error!("Invalid value '{}' for field '{}'", value, field);
                    }
                    ConfigError::ValidationFailed(msg) => {
                        log::error!("Configuration validation failed: {}", msg);
                    }
                    _ => {}
                }
                Err(e)
            }
        }
    }

    /// Print configuration template for environment variables
    pub fn print_config_template() {
        println!("# Discord Spotify Bot Configuration Template");
        println!("# Copy these environment variables and set appropriate values");
        println!();
        println!("export DISCORD_TOKEN=\"your_discord_bot_token_here\"");
        println!("export SPOTIFY_CLIENT_ID=\"your_spotify_client_id_here\"");
        println!("export SPOTIFY_CLIENT_SECRET=\"your_spotify_client_secret_here\"");
        println!("export SPOTIFY_REFRESH_TOKEN=\"your_spotify_refresh_token_here\"");
        println!("export TARGET_CHANNEL_ID=\"123456789012345678\"");
        println!("export COLLABORATIVE_PLAYLIST_ID=\"your_collaborative_playlist_id_here\"");
        println!("export DISCOVERY_PLAYLIST_ID=\"your_discovery_playlist_id_here\"");
        println!();
        println!("# Optional configuration (with defaults):");
        println!("export WEEKLY_SCHEDULE_CRON=\"0 0 12 * * MON\"  # Every Monday at noon");
        println!("export MAX_RETRY_ATTEMPTS=\"3\"");
        println!("export RETRY_BASE_DELAY_MS=\"1000\"");
        println!("export RETRY_MAX_DELAY_MS=\"30000\"");
    }
}