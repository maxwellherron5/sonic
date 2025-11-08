use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Core track information from Spotify
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackInfo {
    /// Spotify track ID
    pub id: String,
    /// Spotify track URI (e.g., "spotify:track:...")
    pub uri: String,
    /// Track name
    pub name: String,
    /// List of artist names
    pub artists: Vec<String>,
    /// Album name
    pub album: String,
    /// Track duration in milliseconds
    pub duration_ms: u32,
    /// External URLs (e.g., Spotify web player URL)
    pub external_urls: HashMap<String, String>,
    /// Track popularity (0-100)
    pub popularity: Option<u8>,
    /// Preview URL for 30-second track preview
    pub preview_url: Option<String>,
    /// Explicit content flag
    pub explicit: bool,
}

impl TrackInfo {
    /// Create a new TrackInfo instance
    pub fn new(
        id: String,
        uri: String,
        name: String,
        artists: Vec<String>,
        album: String,
        duration_ms: u32,
    ) -> Self {
        Self {
            id,
            uri,
            name,
            artists,
            album,
            duration_ms,
            external_urls: HashMap::new(),
            popularity: None,
            preview_url: None,
            explicit: false,
        }
    }

    /// Get the primary artist name
    pub fn primary_artist(&self) -> Option<&String> {
        self.artists.first()
    }

    /// Get all artists as a comma-separated string
    pub fn artists_string(&self) -> String {
        self.artists.join(", ")
    }

    /// Get duration in minutes and seconds format
    pub fn duration_formatted(&self) -> String {
        let total_seconds = self.duration_ms / 1000;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{}:{:02}", minutes, seconds)
    }
}

/// Statistics for a playlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistStats {
    /// Total number of tracks in the playlist
    pub total_tracks: usize,
    /// Number of unique artists
    pub unique_artists: usize,
    /// Total duration of all tracks in milliseconds
    pub total_duration_ms: u64,
    /// When the playlist was last updated
    pub last_updated: SystemTime,
    /// Average track popularity (if available)
    pub average_popularity: Option<f32>,
    /// Most common artist in the playlist
    pub most_common_artist: Option<String>,
    /// Number of explicit tracks
    pub explicit_tracks: usize,
}

impl PlaylistStats {
    /// Create new playlist statistics
    pub fn new() -> Self {
        Self {
            total_tracks: 0,
            unique_artists: 0,
            total_duration_ms: 0,
            last_updated: SystemTime::now(),
            average_popularity: None,
            most_common_artist: None,
            explicit_tracks: 0,
        }
    }

    /// Calculate statistics from a list of tracks
    pub fn from_tracks(tracks: &[TrackInfo]) -> Self {
        let total_tracks = tracks.len();
        let total_duration_ms: u64 = tracks.iter().map(|t| t.duration_ms as u64).sum();
        
        let mut artist_counts: HashMap<String, usize> = HashMap::new();
        let mut popularity_sum = 0u32;
        let mut popularity_count = 0usize;
        let mut explicit_tracks = 0usize;

        for track in tracks {
            // Count artists
            for artist in &track.artists {
                *artist_counts.entry(artist.clone()).or_insert(0) += 1;
            }

            // Calculate average popularity
            if let Some(popularity) = track.popularity {
                popularity_sum += popularity as u32;
                popularity_count += 1;
            }

            // Count explicit tracks
            if track.explicit {
                explicit_tracks += 1;
            }
        }

        let unique_artists = artist_counts.len();
        let average_popularity = if popularity_count > 0 {
            Some(popularity_sum as f32 / popularity_count as f32)
        } else {
            None
        };

        let most_common_artist = artist_counts
            .iter()
            .max_by_key(|(_, &count)| count)
            .map(|(artist, _)| artist.clone());

        Self {
            total_tracks,
            unique_artists,
            total_duration_ms,
            last_updated: SystemTime::now(),
            average_popularity,
            most_common_artist,
            explicit_tracks,
        }
    }

    /// Get total duration in hours, minutes, and seconds format
    pub fn duration_formatted(&self) -> String {
        let total_seconds = self.total_duration_ms / 1000;
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

impl Default for PlaylistStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for the Discord Spotify Bot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    /// Discord bot token
    pub discord_token: String,
    /// Spotify client ID
    pub spotify_client_id: String,
    /// Spotify client secret
    pub spotify_client_secret: String,
    /// Spotify refresh token for long-term access
    pub spotify_refresh_token: String,
    /// Discord channel ID to monitor for Spotify links
    pub target_channel_id: u64,
    /// Spotify playlist ID for the collaborative playlist
    pub collaborative_playlist_id: String,
    /// Spotify playlist ID for the discovery playlist
    pub discovery_playlist_id: String,
    /// Cron expression for weekly discovery playlist updates
    pub weekly_schedule_cron: String,
    /// Maximum number of retry attempts for API calls
    pub max_retry_attempts: u32,
    /// Base retry delay in milliseconds
    pub retry_base_delay_ms: u64,
    /// Maximum retry delay in milliseconds
    pub retry_max_delay_ms: u64,
}

impl BotConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self {
            discord_token: String::new(),
            spotify_client_id: String::new(),
            spotify_client_secret: String::new(),
            spotify_refresh_token: String::new(),
            target_channel_id: 0,
            collaborative_playlist_id: String::new(),
            discovery_playlist_id: String::new(),
            weekly_schedule_cron: "0 0 12 * * MON".to_string(), // Every Monday at noon
            max_retry_attempts: 3,
            retry_base_delay_ms: 1000,
            retry_max_delay_ms: 30000,
        }
    }

    /// Validate that all required fields are set
    pub fn validate(&self) -> Result<(), String> {
        if self.discord_token.is_empty() {
            return Err("Discord token is required".to_string());
        }
        if self.spotify_client_id.is_empty() {
            return Err("Spotify client ID is required".to_string());
        }
        if self.spotify_client_secret.is_empty() {
            return Err("Spotify client secret is required".to_string());
        }
        if self.spotify_refresh_token.is_empty() {
            return Err("Spotify refresh token is required".to_string());
        }
        if self.target_channel_id == 0 {
            return Err("Target channel ID is required".to_string());
        }
        if self.collaborative_playlist_id.is_empty() {
            return Err("Collaborative playlist ID is required".to_string());
        }
        if self.discovery_playlist_id.is_empty() {
            return Err("Discovery playlist ID is required".to_string());
        }
        if self.max_retry_attempts == 0 {
            return Err("Max retry attempts must be greater than 0".to_string());
        }
        if self.retry_base_delay_ms == 0 {
            return Err("Retry base delay must be greater than 0".to_string());
        }

        Ok(())
    }
}

impl Default for BotConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of adding a track to a playlist
#[derive(Debug, Clone)]
pub enum AddTrackResult {
    /// Track was successfully added
    Added(TrackInfo),
    /// Track already exists in the playlist
    AlreadyExists(TrackInfo),
    /// Failed to add track
    Failed(String),
}

/// Discovery playlist generation result
#[derive(Debug, Clone)]
pub struct DiscoveryPlaylist {
    /// Generated tracks for the discovery playlist
    pub tracks: Vec<TrackInfo>,
    /// When the playlist was generated
    pub generated_at: SystemTime,
    /// Seed tracks used for recommendations
    pub seed_tracks: Vec<String>,
    /// Statistics about the generated playlist
    pub stats: PlaylistStats,
}

impl DiscoveryPlaylist {
    /// Create a new discovery playlist
    pub fn new(tracks: Vec<TrackInfo>, seed_tracks: Vec<String>) -> Self {
        let stats = PlaylistStats::from_tracks(&tracks);
        Self {
            tracks,
            generated_at: SystemTime::now(),
            seed_tracks,
            stats,
        }
    }

    /// Get the number of tracks in the discovery playlist
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    /// Check if the playlist has the target number of tracks (20)
    pub fn is_complete(&self) -> bool {
        self.tracks.len() == 20
    }
}

/// Spotify URL types that can be processed
#[derive(Debug, Clone, PartialEq)]
pub enum SpotifyUrlType {
    /// Track URL with track ID
    Track(String),
    /// Album URL with album ID
    Album(String),
    /// Playlist URL with playlist ID
    Playlist(String),
    /// Artist URL with artist ID
    Artist(String),
    /// Unsupported URL type
    Unsupported,
}

impl SpotifyUrlType {
    /// Check if the URL type is supported for adding to playlists
    pub fn is_addable(&self) -> bool {
        matches!(self, SpotifyUrlType::Track(_))
    }

    /// Get the ID from the URL type
    pub fn id(&self) -> Option<&String> {
        match self {
            SpotifyUrlType::Track(id) 
            | SpotifyUrlType::Album(id) 
            | SpotifyUrlType::Playlist(id) 
            | SpotifyUrlType::Artist(id) => Some(id),
            SpotifyUrlType::Unsupported => None,
        }
    }
}