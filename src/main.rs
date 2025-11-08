use sonic::config::utils::load_config_with_details;
use sonic::discord_client::start_bot_with_scheduler;
use tokio::signal;
use std::time::SystemTime;

#[tokio::main]
async fn main() {
    // Load .env file if it exists
    let _ = dotenv::dotenv();
    
    // Initialize logging
    env_logger::init();
    
    let _startup_time = SystemTime::now();

    // Load and validate configuration
    let config = match load_config_with_details() {
        Ok(config) => {
            log::info!("Starting Discord Spotify Bot with scheduler...");
            config
        }
        Err(e) => {
            log::error!("Failed to start bot due to configuration error: {}", e);
            std::process::exit(1);
        }
    };

    // Start the bot with scheduler integration
    let bot_handle = tokio::spawn(async move {
        start_bot_with_scheduler(config).await;
    });

    // Set up graceful shutdown handling
    let shutdown_signal = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        log::info!("Received shutdown signal, stopping bot...");
    };

    // Wait for either the bot to finish or a shutdown signal
    tokio::select! {
        _ = bot_handle => {
            log::info!("Bot task completed");
        }
        _ = shutdown_signal => {
            log::info!("Shutdown signal received, terminating...");
        }
    }

    log::info!("Discord Spotify Bot shutdown complete");
}
