use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::{PlaylistError, PlaylistResult, SpotifyError};
use crate::models::{AddTrackResult, BotConfig, PlaylistStats, TrackInfo};
use crate::spotify_client::SpotifyClient;

/// High-level playlist operations with business logic for duplicate prevention and playlist maintenance
pub struct PlaylistManager {
    spotify_client: Arc<Mutex<SpotifyClient>>,
    config: BotConfig,
}

impl PlaylistManager {
    /// Create a new PlaylistManager instance
    pub fn new(spotify_client: Arc<Mutex<SpotifyClient>>, config: BotConfig) -> Self {
        Self {
            spotify_client,
            config,
        }
    }

    /// Add a track to the collaborative playlist with duplicate checking
    /// Returns the result of the operation including track information
    pub async fn add_track_to_collaborative(&self, track_uri: &str) -> PlaylistResult<AddTrackResult> {
        let mut client = self.spotify_client.lock().await;
        
        // Extract track ID from URI for getting track info
        let track_id = self.extract_track_id_from_uri(track_uri)?;
        
        // Get track information first
        let track_info = client.get_track_info(&track_id).await
            .map_err(|e| match e {
                SpotifyError::TrackNotFound { track_id } => {
                    PlaylistError::AddTrackFailed(format!("Track not found: {}", track_id))
                }
                _ => PlaylistError::AddTrackFailed(format!("{:?}", e))
            })?;

        // Check if track already exists in the collaborative playlist
        let exists = client.check_track_exists_in_playlist(&self.config.collaborative_playlist_id, track_uri).await
            .map_err(|e| PlaylistError::AddTrackFailed(format!("Failed to check for duplicates: {:?}", e)))?;

        if exists {
            log::info!("Track '{}' by {} already exists in collaborative playlist", 
                      track_info.name, track_info.artists_string());
            return Ok(AddTrackResult::AlreadyExists(track_info));
        }

        // Add track to playlist
        match client.add_track_to_playlist(&self.config.collaborative_playlist_id, track_uri).await {
            Ok(()) => {
                log::info!("Successfully added track '{}' by {} to collaborative playlist", 
                          track_info.name, track_info.artists_string());
                Ok(AddTrackResult::Added(track_info))
            }
            Err(e) => {
                let error_msg = format!("Failed to add track '{}': {:?}", track_info.name, e);
                log::error!("{}", error_msg);
                Ok(AddTrackResult::Failed(error_msg))
            }
        }
    }

    /// Get all tracks from the collaborative playlist
    pub async fn get_collaborative_tracks(&self) -> PlaylistResult<Vec<TrackInfo>> {
        let mut client = self.spotify_client.lock().await;
        
        client.get_playlist_tracks(&self.config.collaborative_playlist_id).await
            .map_err(|e| PlaylistError::RetrieveTracksFailed(format!(
                "Failed to retrieve tracks from collaborative playlist: {:?}", e
            )))
    }

    /// Get statistics for the collaborative playlist
    pub async fn get_collaborative_playlist_stats(&self) -> PlaylistResult<PlaylistStats> {
        let tracks = self.get_collaborative_tracks().await?;
        Ok(PlaylistStats::from_tracks(&tracks))
    }

    /// Get tracks from the discovery playlist
    pub async fn get_discovery_tracks(&self) -> PlaylistResult<Vec<TrackInfo>> {
        let mut client = self.spotify_client.lock().await;
        
        client.get_playlist_tracks(&self.config.discovery_playlist_id).await
            .map_err(|e| PlaylistError::RetrieveTracksFailed(format!(
                "Failed to retrieve tracks from discovery playlist: {:?}", e
            )))
    }

    /// Get statistics for the discovery playlist
    pub async fn get_discovery_playlist_stats(&self) -> PlaylistResult<PlaylistStats> {
        let tracks = self.get_discovery_tracks().await?;
        Ok(PlaylistStats::from_tracks(&tracks))
    }

    /// Replace all tracks in the discovery playlist with new tracks
    pub async fn replace_discovery_playlist(&self, track_uris: Vec<String>) -> PlaylistResult<()> {
        if track_uris.is_empty() {
            return Err(PlaylistError::ReplaceTracksFailed(
                "Cannot replace playlist with empty track list".to_string()
            ));
        }

        let track_count = track_uris.len();
        let mut client = self.spotify_client.lock().await;
        
        client.replace_playlist_tracks(&self.config.discovery_playlist_id, track_uris).await
            .map_err(|e| PlaylistError::ReplaceTracksFailed(format!(
                "Failed to replace discovery playlist tracks: {:?}", e
            )))?;

        log::info!("Successfully replaced discovery playlist with {} tracks", track_count);
        Ok(())
    }

    /// Add multiple tracks to the collaborative playlist
    /// Returns a vector of results for each track
    pub async fn add_multiple_tracks_to_collaborative(&self, track_uris: Vec<String>) -> PlaylistResult<Vec<AddTrackResult>> {
        let mut results = Vec::new();
        
        for track_uri in track_uris {
            let result = self.add_track_to_collaborative(&track_uri).await?;
            results.push(result);
        }
        
        Ok(results)
    }

    /// Get recent tracks from the collaborative playlist (last N tracks)
    pub async fn get_recent_collaborative_tracks(&self, limit: usize) -> PlaylistResult<Vec<TrackInfo>> {
        let all_tracks = self.get_collaborative_tracks().await?;
        
        // Return the last N tracks (most recently added)
        let recent_tracks = if all_tracks.len() <= limit {
            all_tracks
        } else {
            all_tracks.into_iter().rev().take(limit).rev().collect()
        };
        
        Ok(recent_tracks)
    }

    /// Check if a track exists in the collaborative playlist by track ID
    pub async fn track_exists_in_collaborative(&self, track_id: &str) -> PlaylistResult<bool> {
        let track_uri = format!("spotify:track:{}", track_id);
        let mut client = self.spotify_client.lock().await;
        
        client.check_track_exists_in_playlist(&self.config.collaborative_playlist_id, &track_uri).await
            .map_err(|e| PlaylistError::RetrieveTracksFailed(format!(
                "Failed to check if track exists: {:?}", e
            )))
    }

    /// Get a summary of both playlists for reporting
    pub async fn get_playlists_summary(&self) -> PlaylistResult<PlaylistsSummary> {
        let collaborative_stats = self.get_collaborative_playlist_stats().await?;
        let discovery_stats = self.get_discovery_playlist_stats().await?;
        
        Ok(PlaylistsSummary {
            collaborative: collaborative_stats,
            discovery: discovery_stats,
        })
    }

    /// Extract track ID from Spotify URI
    fn extract_track_id_from_uri(&self, track_uri: &str) -> PlaylistResult<String> {
        if let Some(track_id) = track_uri.strip_prefix("spotify:track:") {
            Ok(track_id.to_string())
        } else {
            Err(PlaylistError::AddTrackFailed(format!(
                "Invalid Spotify track URI format: {}", track_uri
            )))
        }
    }

    /// Validate that a track URI is properly formatted
    pub fn validate_track_uri(&self, track_uri: &str) -> PlaylistResult<()> {
        if !track_uri.starts_with("spotify:track:") {
            return Err(PlaylistError::AddTrackFailed(format!(
                "Invalid track URI format: {}. Expected format: spotify:track:TRACK_ID", track_uri
            )));
        }

        let track_id = track_uri.strip_prefix("spotify:track:").unwrap();
        if track_id.is_empty() || track_id.len() != 22 {
            return Err(PlaylistError::AddTrackFailed(format!(
                "Invalid track ID in URI: {}. Track ID should be 22 characters long", track_uri
            )));
        }

        Ok(())
    }

    /// Get the collaborative playlist ID
    pub fn get_collaborative_playlist_id(&self) -> &str {
        &self.config.collaborative_playlist_id
    }

    /// Get the discovery playlist ID
    pub fn get_discovery_playlist_id(&self) -> &str {
        &self.config.discovery_playlist_id
    }
}

/// Summary of both playlists for reporting purposes
#[derive(Debug, Clone)]
pub struct PlaylistsSummary {
    /// Statistics for the collaborative playlist
    pub collaborative: PlaylistStats,
    /// Statistics for the discovery playlist
    pub discovery: PlaylistStats,
}

impl PlaylistsSummary {
    /// Get a formatted string representation of the summary
    pub fn format_summary(&self) -> String {
        format!(
            "📊 **Playlist Summary**\n\
            🎵 **Collaborative Playlist:**\n\
            • {} tracks from {} unique artists\n\
            • Total duration: {}\n\
            • {} explicit tracks\n\
            • Most common artist: {}\n\
            • Last updated: {:?}\n\n\
            🔍 **Discovery Playlist:**\n\
            • {} tracks from {} unique artists\n\
            • Total duration: {}\n\
            • {} explicit tracks\n\
            • Most common artist: {}\n\
            • Last updated: {:?}",
            self.collaborative.total_tracks,
            self.collaborative.unique_artists,
            self.collaborative.duration_formatted(),
            self.collaborative.explicit_tracks,
            self.collaborative.most_common_artist.as_deref().unwrap_or("None"),
            self.collaborative.last_updated,
            self.discovery.total_tracks,
            self.discovery.unique_artists,
            self.discovery.duration_formatted(),
            self.discovery.explicit_tracks,
            self.discovery.most_common_artist.as_deref().unwrap_or("None"),
            self.discovery.last_updated
        )
    }

    /// Get total tracks across both playlists
    pub fn total_tracks(&self) -> usize {
        self.collaborative.total_tracks + self.discovery.total_tracks
    }

    /// Get total unique artists across both playlists
    pub fn total_unique_artists(&self) -> usize {
        // Note: This is an approximation since we don't deduplicate across playlists
        self.collaborative.unique_artists + self.discovery.unique_artists
    }

    /// Get total duration across both playlists
    pub fn total_duration_ms(&self) -> u64 {
        self.collaborative.total_duration_ms + self.discovery.total_duration_ms
    }

    /// Get formatted total duration
    pub fn total_duration_formatted(&self) -> String {
        let total_seconds = self.total_duration_ms() / 1000;
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else {
            format!("{}m {}s", minutes, seconds)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_track_id_from_uri() {
        let config = BotConfig::default();
        let spotify_client = Arc::new(Mutex::new(SpotifyClient::new(&config)));
        let manager = PlaylistManager::new(spotify_client, config);

        // Valid URI
        let result = manager.extract_track_id_from_uri("spotify:track:4iV5W9uYEdYUVa79Axb7Rh");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "4iV5W9uYEdYUVa79Axb7Rh");

        // Invalid URI
        let result = manager.extract_track_id_from_uri("invalid:uri:format");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_track_uri() {
        let config = BotConfig::default();
        let spotify_client = Arc::new(Mutex::new(SpotifyClient::new(&config)));
        let manager = PlaylistManager::new(spotify_client, config);

        // Valid URI
        assert!(manager.validate_track_uri("spotify:track:4iV5W9uYEdYUVa79Axb7Rh").is_ok());

        // Invalid format
        assert!(manager.validate_track_uri("invalid:format").is_err());

        // Empty track ID
        assert!(manager.validate_track_uri("spotify:track:").is_err());

        // Wrong length track ID
        assert!(manager.validate_track_uri("spotify:track:short").is_err());
    }

    #[test]
    fn test_playlists_summary_formatting() {
        let collaborative_stats = PlaylistStats {
            total_tracks: 50,
            unique_artists: 25,
            total_duration_ms: 180000, // 3 minutes
            last_updated: std::time::SystemTime::now(),
            average_popularity: Some(75.0),
            most_common_artist: Some("Test Artist".to_string()),
            explicit_tracks: 5,
        };

        let discovery_stats = PlaylistStats {
            total_tracks: 20,
            unique_artists: 15,
            total_duration_ms: 120000, // 2 minutes
            last_updated: std::time::SystemTime::now(),
            average_popularity: Some(80.0),
            most_common_artist: Some("Another Artist".to_string()),
            explicit_tracks: 2,
        };

        let summary = PlaylistsSummary {
            collaborative: collaborative_stats,
            discovery: discovery_stats,
        };

        assert_eq!(summary.total_tracks(), 70);
        assert_eq!(summary.total_duration_ms(), 300000);
        assert_eq!(summary.total_duration_formatted(), "5m 0s");

        let formatted = summary.format_summary();
        assert!(formatted.contains("50 tracks"));
        assert!(formatted.contains("20 tracks"));
        assert!(formatted.contains("Test Artist"));
        assert!(formatted.contains("Another Artist"));
    }
}