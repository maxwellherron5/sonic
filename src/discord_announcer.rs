use serenity::model::id::ChannelId;
use std::sync::Arc;

use crate::error::{DiscordError, DiscordResult};
use crate::models::{BotConfig, DiscoveryPlaylist};

/// Service for sending Discord announcements
/// This allows other components to send messages to Discord without direct coupling
pub struct DiscordAnnouncer {
    http: Arc<serenity::http::Http>,
    config: BotConfig,
}

impl DiscordAnnouncer {
    /// Create a new DiscordAnnouncer instance
    pub fn new(http: Arc<serenity::http::Http>, config: BotConfig) -> Self {
        Self {
            http,
            config,
        }
    }

    /// Send discovery playlist announcement to the target channel
    /// Implements requirement 4.5: announce new discovery playlist in target channel
    pub async fn announce_discovery_playlist(&self, discovery_playlist: &DiscoveryPlaylist) -> DiscordResult<()> {
        let channel_id = ChannelId(self.config.target_channel_id);
        
        // Format the announcement message with playlist statistics and generation timestamp
        let announcement = self.format_discovery_announcement(discovery_playlist);
        
        // Send the announcement message
        match channel_id.say(&self.http, &announcement).await {
            Ok(_) => {
                log::info!("Successfully announced new discovery playlist to channel {}", self.config.target_channel_id);
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to announce discovery playlist to channel {}: {}", self.config.target_channel_id, e);
                Err(DiscordError::MessageSendFailed(format!(
                    "Failed to send discovery playlist announcement: {}", e
                )))
            }
        }
    }

    /// Format the discovery playlist announcement message
    /// Includes playlist statistics and generation timestamp as required
    fn format_discovery_announcement(&self, discovery_playlist: &DiscoveryPlaylist) -> String {
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
    pub async fn send_announcement(&self, message: &str) -> DiscordResult<()> {
        let channel_id = ChannelId(self.config.target_channel_id);
        
        match channel_id.say(&self.http, message).await {
            Ok(_) => {
                log::info!("Successfully sent announcement to channel {}", self.config.target_channel_id);
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to send announcement to channel {}: {}", self.config.target_channel_id, e);
                Err(DiscordError::MessageSendFailed(format!(
                    "Failed to send announcement: {}", e
                )))
            }
        }
    }

    /// Send error announcement when discovery playlist generation fails
    pub async fn announce_discovery_error(&self, error: &str) -> DiscordResult<()> {
        let error_message = format!(
            "⚠️ **Discovery Playlist Generation Failed**\n\n\
            An error occurred while generating this week's discovery playlist:\n\
            ```\n{}\n```\n\n\
            The bot will try again during the next scheduled generation.",
            error
        );
        
        self.send_announcement(&error_message).await
    }

    /// Send success announcement for manual discovery playlist generation
    pub async fn announce_manual_discovery_success(&self, track_count: usize) -> DiscordResult<()> {
        let success_message = format!(
            "✅ **Manual Discovery Playlist Generated**\n\n\
            Successfully generated a new discovery playlist with {} tracks!\n\
            🎧 Check it out: https://open.spotify.com/playlist/{}",
            track_count,
            self.config.discovery_playlist_id
        );
        
        self.send_announcement(&success_message).await
    }

    /// Get the target channel ID
    pub fn get_target_channel_id(&self) -> u64 {
        self.config.target_channel_id
    }

    /// Get the discovery playlist ID
    pub fn get_discovery_playlist_id(&self) -> &str {
        &self.config.discovery_playlist_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TrackInfo;
    use std::collections::HashMap;

    fn create_test_discovery_playlist() -> DiscoveryPlaylist {
        let tracks = vec![
            TrackInfo {
                id: "1".to_string(),
                uri: "spotify:track:1".to_string(),
                name: "Test Track 1".to_string(),
                artists: vec!["Artist 1".to_string()],
                album: "Album 1".to_string(),
                duration_ms: 180000,
                external_urls: HashMap::new(),
                popularity: Some(75),
                preview_url: None,
                explicit: false,
            },
            TrackInfo {
                id: "2".to_string(),
                uri: "spotify:track:2".to_string(),
                name: "Test Track 2".to_string(),
                artists: vec!["Artist 2".to_string()],
                album: "Album 2".to_string(),
                duration_ms: 200000,
                external_urls: HashMap::new(),
                popularity: Some(80),
                preview_url: None,
                explicit: true,
            },
        ];

        let seed_tracks = vec!["seed1".to_string(), "seed2".to_string()];
        DiscoveryPlaylist::new(tracks, seed_tracks)
    }

    #[test]
    fn test_format_discovery_announcement() {
        let config = BotConfig {
            discord_token: "test".to_string(),
            spotify_client_id: "test".to_string(),
            spotify_client_secret: "test".to_string(),
            spotify_refresh_token: "test".to_string(),
            target_channel_id: 123456789,
            collaborative_playlist_id: "collab123".to_string(),
            discovery_playlist_id: "discovery123".to_string(),
            weekly_schedule_cron: "0 0 12 * * MON".to_string(),
            max_retry_attempts: 3,
            retry_base_delay_ms: 1000,
            retry_max_delay_ms: 30000,
        };

        // Create a mock HTTP client (we won't actually use it in this test)
        let http = Arc::new(serenity::http::Http::new("fake_token"));
        let announcer = DiscordAnnouncer::new(http, config);
        
        let discovery_playlist = create_test_discovery_playlist();
        let announcement = announcer.format_discovery_announcement(&discovery_playlist);
        
        // Verify the announcement contains expected elements
        assert!(announcement.contains("New Discovery Playlist is Ready!"));
        assert!(announcement.contains("2 tracks"));
        assert!(announcement.contains("2 unique artists"));
        assert!(announcement.contains("1 explicit tracks"));
        assert!(announcement.contains("2 seed tracks"));
        assert!(announcement.contains("discovery123"));
        assert!(announcement.contains("https://open.spotify.com/playlist/"));
    }

    #[test]
    fn test_announcer_getters() {
        let config = BotConfig {
            discord_token: "test".to_string(),
            spotify_client_id: "test".to_string(),
            spotify_client_secret: "test".to_string(),
            spotify_refresh_token: "test".to_string(),
            target_channel_id: 123456789,
            collaborative_playlist_id: "collab123".to_string(),
            discovery_playlist_id: "discovery123".to_string(),
            weekly_schedule_cron: "0 0 12 * * MON".to_string(),
            max_retry_attempts: 3,
            retry_base_delay_ms: 1000,
            retry_max_delay_ms: 30000,
        };

        let http = Arc::new(serenity::http::Http::new("fake_token"));
        let announcer = DiscordAnnouncer::new(http, config);
        
        assert_eq!(announcer.get_target_channel_id(), 123456789);
        assert_eq!(announcer.get_discovery_playlist_id(), "discovery123");
    }
}