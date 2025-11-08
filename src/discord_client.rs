use log::{error, info, warn};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::{ConfigManager, DefaultConfigManager};
use crate::message_processor::MessageProcessor;
use crate::models::BotConfig;
use crate::spotify_client::SpotifyClient;

struct Handler {
    spotify_client: Arc<Mutex<SpotifyClient>>,
    message_processor: MessageProcessor,
    config: BotConfig,
}

impl Handler {
    async fn new(config: BotConfig) -> Handler {
        let mut spotify_client = SpotifyClient::new(&config);
        
        // Initialize the Spotify client
        if let Err(e) = spotify_client.initialize().await {
            error!("Failed to initialize Spotify client: {}", e);
        }
        
        Handler {
            spotify_client: Arc::new(Mutex::new(spotify_client)),
            message_processor: MessageProcessor::new(),
            config,
        }
    }

    /// Check if the message is from the target channel
    fn is_target_channel(&self, channel_id: u64) -> bool {
        channel_id == self.config.target_channel_id
    }

    /// Validate that the target channel is accessible and properly configured
    async fn validate_target_channel(&self, ctx: &Context) -> Result<(), crate::error::DiscordError> {
        use crate::error::DiscordError;
        
        // Try to get the channel to verify it exists and is accessible
        match ctx.http.get_channel(self.config.target_channel_id).await {
            Ok(_) => {
                info!("Target channel {} is accessible", self.config.target_channel_id);
                Ok(())
            }
            Err(e) => {
                error!("Target channel {} is not accessible: {}", self.config.target_channel_id, e);
                Err(DiscordError::ChannelNotFound { 
                    channel_id: self.config.target_channel_id 
                })
            }
        }
    }

    /// Send feedback message to the channel with error handling
    async fn send_feedback(&self, ctx: &Context, msg: &Message, content: String) {
        if let Err(e) = msg.channel_id.say(&ctx.http, &content).await {
            warn!("Failed to send feedback message '{}': {}", content, e);
            
            // Try to send a simplified fallback message
            let fallback = "⚠️ Operation completed but couldn't send detailed feedback.";
            if let Err(e) = msg.channel_id.say(&ctx.http, fallback).await {
                error!("Failed to send fallback feedback message: {}", e);
            }
        }
    }

    /// Send success feedback for track addition
    async fn send_success_feedback(&self, ctx: &Context, msg: &Message, track_info: &crate::models::TrackInfo) {
        let confirmation_msg = format!(
            "✅ **Added to playlist!**\n🎵 **{}** by **{}**\n💿 Album: {}\n⏱️ Duration: {}",
            track_info.name,
            track_info.artists_string(),
            track_info.album,
            track_info.duration_formatted()
        );
        
        self.send_feedback(ctx, msg, confirmation_msg).await;
    }

    /// Send error feedback with appropriate context
    async fn send_error_feedback(&self, ctx: &Context, msg: &Message, _error: &str, error_type: &str) {
        let error_msg = match error_type {
            "duplicate" => format!("🔄 This track is already in the playlist!"),
            "rate_limit" => "⏳ Spotify rate limit reached. Please wait a moment and try again.".to_string(),
            "permission" => "🔒 Permission denied. Please check playlist access settings.".to_string(),
            "network" => "🌐 Network error. Please check your connection and try again.".to_string(),
            "not_found" => "❌ Spotify track not found. The track may have been removed or the link is incorrect.".to_string(),
            "authentication" => "🔑 Authentication error. Please contact the bot administrator.".to_string(),
            "invalid_url" => "❌ Invalid Spotify track link. Please check the URL and try again.".to_string(),
            _ => "❌ An error occurred. Please try again later.".to_string(),
        };
        
        self.send_feedback(ctx, msg, error_msg).await;
    }

    /// Send discovery playlist announcement to the target channel
    /// Implements requirement 4.5: announce new discovery playlist in target channel
    pub async fn announce_discovery_playlist(&self, ctx: &Context, discovery_playlist: &crate::models::DiscoveryPlaylist) -> Result<(), crate::error::DiscordError> {
        use crate::error::DiscordError;
        use serenity::model::id::ChannelId;
        
        let channel_id = ChannelId(self.config.target_channel_id);
        
        // Format the announcement message with playlist statistics and generation timestamp
        let announcement = self.format_discovery_announcement(discovery_playlist);
        
        // Send the announcement message
        match channel_id.say(&ctx.http, &announcement).await {
            Ok(_) => {
                info!("Successfully announced new discovery playlist to channel {}", self.config.target_channel_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to announce discovery playlist to channel {}: {}", self.config.target_channel_id, e);
                Err(DiscordError::MessageSendFailed(format!(
                    "Failed to send discovery playlist announcement: {}", e
                )))
            }
        }
    }

    /// Format the discovery playlist announcement message
    /// Includes playlist statistics and generation timestamp as required
    fn format_discovery_announcement(&self, discovery_playlist: &crate::models::DiscoveryPlaylist) -> String {
        use std::time::UNIX_EPOCH;
        
        // Format the generation timestamp
        let timestamp = discovery_playlist.generated_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Create the main announcement message
        let mut announcement = format!(
            "🎵 **New Discovery Playlist is Ready!** 🎵\n\n\
            🔍 **Generated:** <t:{}:F>\n\
            📊 **Playlist Stats:**\n\
            • {} tracks from {} unique artists\n\
            • Total duration: {}\n\
            • {} explicit tracks\n\
            • Generated using {} seed tracks\n\n",
            timestamp,
            discovery_playlist.stats.total_tracks,
            discovery_playlist.stats.unique_artists,
            discovery_playlist.stats.duration_formatted(),
            discovery_playlist.stats.explicit_tracks,
            discovery_playlist.seed_tracks.len()
        );

        // Add most common artist if available
        if let Some(ref artist) = discovery_playlist.stats.most_common_artist {
            announcement.push_str(&format!("🎤 **Most featured artist:** {}\n", artist));
        }

        // Add average popularity if available
        if let Some(popularity) = discovery_playlist.stats.average_popularity {
            announcement.push_str(&format!("⭐ **Average popularity:** {:.1}/100\n", popularity));
        }

        // Add link to discovery playlist
        announcement.push_str(&format!(
            "\n🎧 **Listen now:** https://open.spotify.com/playlist/{}\n\n\
            💡 *This playlist was automatically generated based on recent additions to our collaborative playlist!*",
            self.config.discovery_playlist_id
        ));

        announcement
    }

    /// Send a simple announcement message to the target channel
    /// This is a utility method for sending general announcements
    pub async fn send_announcement(&self, ctx: &Context, message: &str) -> Result<(), crate::error::DiscordError> {
        use crate::error::DiscordError;
        use serenity::model::id::ChannelId;
        
        let channel_id = ChannelId(self.config.target_channel_id);
        
        match channel_id.say(&ctx.http, message).await {
            Ok(_) => {
                info!("Successfully sent announcement to channel {}", self.config.target_channel_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to send announcement to channel {}: {}", self.config.target_channel_id, e);
                Err(DiscordError::MessageSendFailed(format!(
                    "Failed to send announcement: {}", e
                )))
            }
        }
    }

    /// Get track info with retry logic for better error handling
    async fn get_track_info_with_retry(&self, spotify_client: &mut crate::spotify_client::SpotifyClient, track_id: &str) -> Result<crate::models::TrackInfo, crate::error::SpotifyError> {
        let mut attempts = 0;
        let max_attempts = self.config.max_retry_attempts;
        
        loop {
            attempts += 1;
            
            match spotify_client.get_track_info(track_id).await {
                Ok(track_info) => return Ok(track_info),
                Err(e) => {
                    if attempts >= max_attempts {
                        return Err(e);
                    }
                    
                    // Check if error is retryable
                    let should_retry = match &e {
                        crate::error::SpotifyError::RateLimitExceeded { .. } => true,
                        crate::error::SpotifyError::NetworkError(_) => true,
                        crate::error::SpotifyError::ApiRequestFailed { status, .. } => {
                            // Retry on server errors (5xx) but not client errors (4xx)
                            *status >= 500
                        }
                        _ => false,
                    };
                    
                    if !should_retry {
                        return Err(e);
                    }
                    
                    // Calculate delay with exponential backoff
                    let delay_ms = self.config.retry_base_delay_ms * (2_u64.pow(attempts - 1));
                    let delay_ms = delay_ms.min(self.config.retry_max_delay_ms);
                    
                    warn!("Retrying get_track_info for '{}' in {}ms (attempt {}/{}): {}", 
                          track_id, delay_ms, attempts, max_attempts, e);
                    
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    /// Add track to playlist with retry logic for better error handling
    async fn add_track_to_playlist_with_retry(&self, spotify_client: &mut crate::spotify_client::SpotifyClient, track_info: &crate::models::TrackInfo) -> Result<(), crate::error::SpotifyError> {
        let mut attempts = 0;
        let max_attempts = self.config.max_retry_attempts;
        
        loop {
            attempts += 1;
            
            match spotify_client.add_track_to_playlist(&self.config.collaborative_playlist_id, &track_info.uri).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if attempts >= max_attempts {
                        return Err(e);
                    }
                    
                    // Check if error is retryable
                    let should_retry = match &e {
                        crate::error::SpotifyError::RateLimitExceeded { .. } => true,
                        crate::error::SpotifyError::NetworkError(_) => true,
                        crate::error::SpotifyError::ApiRequestFailed { status, .. } => {
                            // Retry on server errors (5xx) but not client errors (4xx)
                            // Exception: don't retry on duplicates (usually 4xx)
                            *status >= 500 && !format!("{:?}", e).contains("already exists")
                        }
                        _ => false,
                    };
                    
                    if !should_retry {
                        return Err(e);
                    }
                    
                    // Calculate delay with exponential backoff
                    let delay_ms = self.config.retry_base_delay_ms * (2_u64.pow(attempts - 1));
                    let delay_ms = delay_ms.min(self.config.retry_max_delay_ms);
                    
                    warn!("Retrying add_track_to_playlist for '{}' in {}ms (attempt {}/{}): {}", 
                          track_info.name, delay_ms, attempts, max_attempts, e);
                    
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }
}

// impl Handler {
//     fn new() -> &'static mut Handler {
//         let mut  spotify_client = spotify_client::SpotifyClient::new();
//         &mut Handler {
//             spotify_client,
//         }
//     }
// }

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Only process messages from non-bots
        if msg.author.bot {
            return;
        }

        // Check if message is from the target channel
        if !self.is_target_channel(msg.channel_id.0) {
            // Log ignored messages from other channels for debugging
            if self.message_processor.extract_spotify_urls(&msg.content).len() > 0 {
                info!("Ignoring Spotify URL from non-target channel {} (target: {})", 
                      msg.channel_id.0, self.config.target_channel_id);
            }
            return;
        }

        // Handle message processing with comprehensive error handling
        if let Err(e) = self.process_message_content(&ctx, &msg).await {
            error!("Failed to process message from user {}: {}", msg.author.name, e);
            
            // Send generic error message to user if processing completely fails
            self.send_error_feedback(&ctx, &msg, &format!("{:?}", e), "general").await;
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
        info!("Monitoring channel ID: {}", self.config.target_channel_id);
        info!("Collaborative playlist ID: {}", self.config.collaborative_playlist_id);

        // Validate target channel accessibility
        if let Err(e) = self.validate_target_channel(&ctx).await {
            error!("Target channel validation failed: {}", e);
            warn!("Bot will continue running but may not function properly until channel is accessible");
        }
    }

}

impl Handler {
    /// Process the content of a message for Spotify URLs
    async fn process_message_content(&self, ctx: &Context, msg: &Message) -> Result<(), crate::error::BotError> {


        // Extract all Spotify URLs from the message with error handling
        let spotify_urls = if msg.content.len() > 2000 {
            // Prevent processing extremely long messages that might cause issues
            warn!("Message too long ({} chars), skipping processing", msg.content.len());
            return Ok(());
        } else {
            self.message_processor.extract_spotify_urls(&msg.content)
        };
        
        if spotify_urls.is_empty() {
            return Ok(());
        }

        info!("Found {} Spotify URL(s) in message from user {} (ID: {})", 
              spotify_urls.len(), msg.author.name, msg.author.id);

        // Process each Spotify URL found in the message
        let mut successful_additions = 0;
        let mut failed_additions = 0;
        
        for (index, url) in spotify_urls.iter().enumerate() {
            info!("Processing Spotify URL {}/{}: {}", index + 1, spotify_urls.len(), url);
            
            match self.process_single_spotify_url(ctx, msg, url).await {
                Ok(true) => {
                    successful_additions += 1;
                }
                Ok(false) => {
                    // URL was processed but not added (e.g., duplicate, non-track URL)
                }
                Err(e) => {
                    failed_additions += 1;
                    error!("Failed to process Spotify URL '{}': {:?}", url, e);
                }
            }
        }

        // Log summary of processing results
        if successful_additions > 0 || failed_additions > 0 {
            info!("Message processing complete for user {}: {} successful, {} failed", 
                  msg.author.name, successful_additions, failed_additions);
        }

        Ok(())
    }

    /// Process a single Spotify URL and return whether it was successfully added
    async fn process_single_spotify_url(&self, ctx: &Context, msg: &Message, url: &str) -> Result<bool, crate::error::BotError> {
        use crate::error::{BotError, MessageProcessingError};

        // Validate and extract track ID with enhanced error handling
        let track_id = match self.message_processor.validate_track_url(url) {
            Ok(id) => id,
            Err(e) => {
                warn!("Invalid or unsupported Spotify URL '{}': {:?}", url, e);
                
                // Only send error message for URLs that look like tracks but are invalid
                if url.contains("/track/") || url.contains("spotify:track:") {
                    self.send_error_feedback(ctx, msg, &format!("{:?}", e), "invalid_url").await;
                    return Err(BotError::MessageProcessing(MessageProcessingError::InvalidSpotifyUrl { 
                        url: url.to_string() 
                    }));
                } else {
                    // For non-track URLs (albums, playlists, etc.), just log and ignore
                    info!("Ignoring non-track Spotify URL: {}", url);
                    return Ok(false);
                }
            }
        };

        // Get Spotify client with timeout protection
        let mut spotify_client = match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.spotify_client.lock()
        ).await {
            Ok(client) => client,
            Err(_) => {
                error!("Timeout while acquiring Spotify client lock");
                self.send_error_feedback(ctx, msg, "Service temporarily unavailable", "general").await;
                return Err(BotError::Spotify(crate::error::SpotifyError::NetworkError(
                    "Client lock timeout".to_string()
                )));
            }
        };

        // Get track info with retry logic
        let track_info = match self.get_track_info_with_retry(&mut spotify_client, &track_id).await {
            Ok(info) => info,
            Err(e) => {
                error!("Failed to get track info for '{}': {:?}", track_id, e);
                
                // Determine error type and send appropriate feedback
                let error_str = format!("{:?}", e);
                let error_type = if error_str.contains("not found") {
                    "not_found"
                } else if error_str.contains("rate limit") {
                    "rate_limit"
                } else if error_str.contains("network") || error_str.contains("timeout") {
                    "network"
                } else if error_str.contains("authentication") || error_str.contains("token") {
                    "authentication"
                } else {
                    "general"
                };
                
                self.send_error_feedback(ctx, msg, &error_str, error_type).await;
                return Err(BotError::Spotify(e));
            }
        };

        // Add track to playlist with retry logic
        match self.add_track_to_playlist_with_retry(&mut spotify_client, &track_info).await {
            Ok(()) => {
                info!("Successfully added track '{}' by {} to collaborative playlist", 
                      track_info.name, track_info.artists_string());
                
                // Send success feedback
                self.send_success_feedback(ctx, msg, &track_info).await;
                Ok(true)
            }
            Err(e) => {
                error!("Failed to add track to playlist: {:?}", e);
                
                // Determine error type and send appropriate feedback
                let error_str = format!("{:?}", e);
                let error_type = if error_str.contains("already exists") {
                    "duplicate"
                } else if error_str.contains("rate limit") {
                    "rate_limit"
                } else if error_str.contains("permission") || error_str.contains("access") {
                    "permission"
                } else if error_str.contains("network") || error_str.contains("timeout") {
                    "network"
                } else {
                    "general"
                };
                
                self.send_error_feedback(ctx, msg, &error_str, error_type).await;
                
                // For duplicates, don't consider it a failure
                if error_type == "duplicate" {
                    Ok(false)
                } else {
                    Err(BotError::Spotify(e))
                }
            }
        }
    }
}

pub async fn start_bot() {
    // Load configuration using the configuration manager
    let config = match DefaultConfigManager::load_config() {
        Ok(config) => {
            info!("Configuration loaded successfully");
            config
        }
        Err(e) => {
            error!("Failed to load configuration: {:?}", e);
            return;
        }
    };

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Create a new instance of the Client, logging in as a bot. This will
    // automatically prepend your bot token with "Bot ", which is a requirement
    // by Discord for bot users.
    let mut client = Client::builder(&config.discord_token, intents)
        .event_handler(Handler::new(config).await)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}

/// Start the bot with integrated scheduler for weekly discovery playlist generation
/// Implements requirements 4.1 and 4.5: schedule weekly discovery generation and announcements
pub async fn start_bot_with_scheduler(config: BotConfig) {
    use crate::discord_announcer::DiscordAnnouncer;
    use crate::discovery_generator::DiscoveryGenerator;
    use crate::playlist_manager::PlaylistManager;
    use crate::scheduler::TaskScheduler;
    use crate::spotify_client::SpotifyClient;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    info!("Initializing Discord Spotify Bot with scheduler...");

    // Initialize Spotify client
    let mut spotify_client = SpotifyClient::new(&config);
    if let Err(e) = spotify_client.initialize().await {
        error!("Failed to initialize Spotify client: {:?}", e);
        return;
    }
    let spotify_client = Arc::new(Mutex::new(spotify_client));

    // Initialize playlist manager
    let playlist_manager = Arc::new(Mutex::new(PlaylistManager::new(
        Arc::clone(&spotify_client),
        config.clone(),
    )));

    // Initialize discovery generator
    let discovery_generator = Arc::new(Mutex::new(DiscoveryGenerator::new(
        Arc::clone(&spotify_client),
        Arc::clone(&playlist_manager),
        config.clone(),
    )));

    // Set gateway intents
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Create Discord client
    let mut client = match Client::builder(&config.discord_token, intents)
        .event_handler(Handler::new(config.clone()).await)
        .await
    {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create Discord client: {}", e);
            return;
        }
    };

    // Get HTTP client for announcements
    let http = Arc::clone(&client.cache_and_http.http);

    // Initialize Discord announcer
    let discord_announcer = Arc::new(Mutex::new(DiscordAnnouncer::new(
        http,
        config.clone(),
    )));

    // Initialize and start the scheduler
    let mut scheduler = match TaskScheduler::new(
        Arc::clone(&discovery_generator),
        Arc::clone(&discord_announcer),
        config.clone(),
    ).await {
        Ok(scheduler) => scheduler,
        Err(e) => {
            error!("Failed to create task scheduler: {:?}", e);
            return;
        }
    };

    // Start the weekly discovery playlist schedule
    if let Err(e) = scheduler.start_weekly_schedule().await {
        error!("Failed to start weekly schedule: {:?}", e);
        return;
    }

    info!("Task scheduler started successfully");
    info!("Weekly discovery playlist generation scheduled with: {}", config.weekly_schedule_cron);

    // Start the Discord client
    info!("Starting Discord client...");
    if let Err(e) = client.start().await {
        error!("Discord client error: {}", e);
        
        // Attempt to stop the scheduler gracefully
        if let Err(scheduler_err) = scheduler.stop().await {
            error!("Failed to stop scheduler during cleanup: {:?}", scheduler_err);
        }
    }

    info!("Discord Spotify Bot with scheduler has stopped");
}
