use log::{error, info, warn};
use sonic::config::utils::load_config_with_details;

fn main() {
    // Load .env file if it exists
    let _ = dotenv::dotenv();
    
    // Initialize logging
    env_logger::init();
    
    info!("Discord Spotify Bot Configuration Validator");
    info!("==========================================");

    // Check if .env file exists
    if std::path::Path::new(".env").exists() {
        info!("✅ .env file found");
    } else {
        warn!("⚠️  .env file not found - using system environment variables");
    }

    // Load and validate configuration
    match load_config_with_details() {
        Ok(config) => {
            info!("✅ Configuration loaded successfully!");
            info!("");
            info!("Configuration Summary:");
            info!("=====================");
            
            // Discord configuration
            info!("Discord Configuration:");
            info!("  Token: {}*** (length: {})", &config.discord_token[..8.min(config.discord_token.len())], config.discord_token.len());
            info!("  Target Channel ID: {}", config.target_channel_id);
            
            // Spotify configuration
            info!("");
            info!("Spotify Configuration:");
            info!("  Client ID: {}", config.spotify_client_id);
            info!("  Client Secret: {}*** (length: {})", &config.spotify_client_secret[..8.min(config.spotify_client_secret.len())], config.spotify_client_secret.len());
            info!("  Refresh Token: {}*** (length: {})", &config.spotify_refresh_token[..8.min(config.spotify_refresh_token.len())], config.spotify_refresh_token.len());
            
            // Playlist configuration
            info!("");
            info!("Playlist Configuration:");
            info!("  Collaborative Playlist ID: {}", config.collaborative_playlist_id);
            info!("  Discovery Playlist ID: {}", config.discovery_playlist_id);
            
            // Optional configuration
            info!("");
            info!("Optional Configuration:");
            info!("  Weekly Schedule: {}", config.weekly_schedule_cron);
            info!("  Max Retry Attempts: {}", config.max_retry_attempts);
            info!("  Base Retry Delay: {}ms", config.retry_base_delay_ms);
            info!("  Max Retry Delay: {}ms", config.retry_max_delay_ms);
            
            // Validation checks
            info!("");
            info!("Validation Checks:");
            info!("=================");
            
            let mut warnings = 0;
            let mut errors = 0;
            
            // Check Discord token format
            if config.discord_token.len() < 50 {
                error!("❌ Discord token appears to be too short (expected ~70 characters)");
                errors += 1;
            } else {
                info!("✅ Discord token format looks correct");
            }
            
            // Check channel ID format
            if config.target_channel_id == 0 {
                error!("❌ Target channel ID is 0 - this is likely incorrect");
                errors += 1;
            } else if config.target_channel_id.to_string().len() < 17 {
                warn!("⚠️  Target channel ID seems short - Discord IDs are usually 17-19 digits");
                warnings += 1;
            } else {
                info!("✅ Target channel ID format looks correct");
            }
            
            // Check Spotify client ID format
            if config.spotify_client_id.len() != 32 {
                warn!("⚠️  Spotify client ID is not 32 characters - this might be incorrect");
                warnings += 1;
            } else {
                info!("✅ Spotify client ID format looks correct");
            }
            
            // Check Spotify client secret format
            if config.spotify_client_secret.len() != 32 {
                warn!("⚠️  Spotify client secret is not 32 characters - this might be incorrect");
                warnings += 1;
            } else {
                info!("✅ Spotify client secret format looks correct");
            }
            
            // Check refresh token format
            if !config.spotify_refresh_token.starts_with("AQ") {
                warn!("⚠️  Spotify refresh token doesn't start with 'AQ' - this might be incorrect");
                warnings += 1;
            } else {
                info!("✅ Spotify refresh token format looks correct");
            }
            
            // Check playlist ID formats
            if config.collaborative_playlist_id.len() != 22 {
                warn!("⚠️  Collaborative playlist ID is not 22 characters - this might be incorrect");
                warnings += 1;
            } else {
                info!("✅ Collaborative playlist ID format looks correct");
            }
            
            if config.discovery_playlist_id.len() != 22 {
                warn!("⚠️  Discovery playlist ID is not 22 characters - this might be incorrect");
                warnings += 1;
            } else {
                info!("✅ Discovery playlist ID format looks correct");
            }
            
            // Check if playlists are the same
            if config.collaborative_playlist_id == config.discovery_playlist_id {
                error!("❌ Collaborative and discovery playlists are the same - they should be different");
                errors += 1;
            } else {
                info!("✅ Collaborative and discovery playlists are different");
            }
            
            // Check cron expression (basic validation)
            let cron_parts: Vec<&str> = config.weekly_schedule_cron.split_whitespace().collect();
            if cron_parts.len() != 6 {
                error!("❌ Cron expression should have 6 parts (second minute hour day month day_of_week)");
                errors += 1;
            } else {
                info!("✅ Cron expression format looks correct");
            }
            
            // Check retry configuration
            if config.max_retry_attempts == 0 {
                warn!("⚠️  Max retry attempts is 0 - no retries will be performed");
                warnings += 1;
            } else if config.max_retry_attempts > 10 {
                warn!("⚠️  Max retry attempts is very high ({}) - this might cause long delays", config.max_retry_attempts);
                warnings += 1;
            } else {
                info!("✅ Retry configuration looks reasonable");
            }
            
            // Summary
            info!("");
            info!("Validation Summary:");
            info!("==================");
            if errors == 0 && warnings == 0 {
                info!("🎉 All configuration checks passed!");
                info!("Your bot should be ready to run.");
            } else {
                if errors > 0 {
                    error!("❌ {} error(s) found - please fix these before running the bot", errors);
                }
                if warnings > 0 {
                    warn!("⚠️  {} warning(s) found - these might cause issues", warnings);
                }
            }
            
            info!("");
            info!("Next Steps:");
            info!("==========");
            info!("1. Run integration test: cargo run --bin integration_test");
            info!("2. If tests pass, start the bot: cargo run --release");
            
            if errors > 0 {
                std::process::exit(1);
            }
        }
        Err(e) => {
            error!("❌ Configuration validation failed: {}", e);
            error!("");
            error!("Common issues:");
            error!("- Missing .env file (copy .env.example to .env)");
            error!("- Missing required environment variables");
            error!("- Invalid environment variable values");
            error!("");
            error!("Required environment variables:");
            error!("- DISCORD_TOKEN");
            error!("- TARGET_CHANNEL_ID");
            error!("- SPOTIFY_CLIENT_ID");
            error!("- SPOTIFY_CLIENT_SECRET");
            error!("- SPOTIFY_REFRESH_TOKEN");
            error!("- COLLABORATIVE_PLAYLIST_ID");
            error!("- DISCOVERY_PLAYLIST_ID");
            
            std::process::exit(1);
        }
    }
}