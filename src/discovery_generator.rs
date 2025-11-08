use rand::seq::SliceRandom;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::{DiscoveryError, DiscoveryResult};
use crate::models::{BotConfig, DiscoveryPlaylist, TrackInfo};
use crate::playlist_manager::PlaylistManager;
use crate::spotify_client::SpotifyClient;

/// Generates weekly discovery playlists using Spotify's recommendation algorithms
pub struct DiscoveryGenerator {
    spotify_client: Arc<Mutex<SpotifyClient>>,
    playlist_manager: Arc<Mutex<PlaylistManager>>,
    config: BotConfig,
}

impl DiscoveryGenerator {
    /// Create a new DiscoveryGenerator instance
    pub fn new(
        spotify_client: Arc<Mutex<SpotifyClient>>,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
        config: BotConfig,
    ) -> Self {
        Self {
            spotify_client,
            playlist_manager,
            config,
        }
    }

    /// Generate a weekly discovery playlist with exactly 20 tracks
    /// Uses collaborative playlist as seed for recommendations
    pub async fn generate_weekly_playlist(&self) -> DiscoveryResult<DiscoveryPlaylist> {
        log::info!("Starting weekly discovery playlist generation");

        // Get all tracks from collaborative playlist
        let collaborative_tracks = {
            let manager = self.playlist_manager.lock().await;
            manager.get_collaborative_tracks().await
                .map_err(|e| DiscoveryError::RecommendationGenerationFailed(
                    format!("Failed to get collaborative tracks: {:?}", e)
                ))?
        };

        if collaborative_tracks.is_empty() {
            return Err(DiscoveryError::InsufficientSeedTracks { 
                count: 0, 
                required: 1 
            });
        }

        // Select seed tracks for recommendations
        let seed_tracks = self.select_seed_tracks(collaborative_tracks).await?;
        
        // Get recommendations from Spotify
        let recommendations = self.get_recommendations(seed_tracks.clone()).await?;
        
        if recommendations.len() < 20 {
            log::warn!("Only got {} recommendations, expected 20", recommendations.len());
        }

        // Take exactly 20 tracks (or all if less than 20)
        let discovery_tracks: Vec<TrackInfo> = recommendations.into_iter().take(20).collect();
        
        let discovery_playlist = DiscoveryPlaylist::new(discovery_tracks, seed_tracks);
        
        log::info!("Generated discovery playlist with {} tracks using {} seed tracks", 
                  discovery_playlist.track_count(), discovery_playlist.seed_tracks.len());
        
        Ok(discovery_playlist)
    }

    /// Select seed tracks from collaborative playlist using random sampling from recent additions
    /// Implements requirement 4.2: use collaborative playlist as seed for recommendations
    pub async fn select_seed_tracks(&self, all_tracks: Vec<TrackInfo>) -> DiscoveryResult<Vec<String>> {
        if all_tracks.is_empty() {
            return Err(DiscoveryError::InsufficientSeedTracks { 
                count: 0, 
                required: 1 
            });
        }

        // Spotify API allows up to 5 seed tracks for recommendations
        const MAX_SEED_TRACKS: usize = 5;
        const RECENT_TRACKS_POOL_SIZE: usize = 50; // Sample from last 50 tracks

        // Get recent tracks (last N tracks added to playlist)
        let recent_tracks = if all_tracks.len() <= RECENT_TRACKS_POOL_SIZE {
            all_tracks
        } else {
            // Take the last RECENT_TRACKS_POOL_SIZE tracks (most recently added)
            all_tracks.into_iter()
                .rev()
                .take(RECENT_TRACKS_POOL_SIZE)
                .collect()
        };

        // Randomly sample seed tracks from recent additions
        let mut rng = rand::thread_rng();
        let seed_count = std::cmp::min(MAX_SEED_TRACKS, recent_tracks.len());
        
        let selected_tracks: Vec<&TrackInfo> = recent_tracks
            .choose_multiple(&mut rng, seed_count)
            .collect();

        let seed_track_ids: Vec<String> = selected_tracks
            .into_iter()
            .map(|track| track.id.clone())
            .collect();

        log::info!("Selected {} seed tracks from {} recent tracks in collaborative playlist", 
                  seed_track_ids.len(), recent_tracks.len());
        
        // Log selected seed tracks for debugging
        for (i, track_id) in seed_track_ids.iter().enumerate() {
            if let Some(track) = recent_tracks.iter().find(|t| t.id == *track_id) {
                log::debug!("Seed track {}: '{}' by {}", 
                           i + 1, track.name, track.artists_string());
            }
        }

        Ok(seed_track_ids)
    }  
  /// Get recommendations using Spotify's search API as a workaround
    /// 
    /// Since the recommendations endpoint is deprecated, this uses the /search endpoint:
    /// 1. For each seed track, search for "artist_name track_name"
    /// 2. Skip the first result (which is the original track)
    /// 3. Collect subsequent results as "similar" tracks
    /// 4. Combine and deduplicate to create a discovery playlist
    pub async fn get_recommendations(&self, seed_tracks: Vec<String>) -> DiscoveryResult<Vec<TrackInfo>> {
        if seed_tracks.is_empty() {
            return Err(DiscoveryError::SeedSelectionFailed(
                "No seed tracks provided for recommendations".to_string()
            ));
        }

        log::info!("Generating recommendations using search-based approach (recommendations API is deprecated)");

        let mut client = self.spotify_client.lock().await;
        let mut discovery_tracks = Vec::new();
        let mut seen_track_ids = std::collections::HashSet::new();

        // For each seed track, use search to find similar tracks
        for seed_track_id in seed_tracks.iter() {
            // Get track info to build search query
            let track_info = match client.get_track_info(seed_track_id).await {
                Ok(info) => info,
                Err(e) => {
                    log::warn!("Failed to get track info for seed {}: {}", seed_track_id, e);
                    continue;
                }
            };

            let artist_name = track_info.artists.first()
                .map(|a| a.as_str())
                .unwrap_or("");
            let track_name = &track_info.name;

            log::debug!("Searching for tracks similar to: {} - {}", artist_name, track_name);

            // Search for similar tracks using artist and track name
            let search_query = format!("{} {}", artist_name, track_name);
            
            match client.search_tracks(&search_query, 10).await {
                Ok(search_results) => {
                    // Skip the first result (likely the original track) and take the rest
                    for track in search_results.into_iter().skip(1) {
                        // Avoid duplicates
                        if seen_track_ids.insert(track.id.clone()) {
                            discovery_tracks.push(track);
                            
                            // Stop if we have enough tracks
                            if discovery_tracks.len() >= 20 {
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Search failed for '{}': {}", search_query, e);
                    continue;
                }
            }

            // Stop if we have enough tracks
            if discovery_tracks.len() >= 20 {
                break;
            }
        }

        if discovery_tracks.is_empty() {
            return Err(DiscoveryError::RecommendationGenerationFailed(
                "Could not generate any recommendations using search API".to_string()
            ));
        }

        log::info!("Generated {} discovery tracks using search-based approach", discovery_tracks.len());

        Ok(discovery_tracks)
    }

    /// Generate and replace the discovery playlist in one operation
    /// This combines generation and playlist replacement for convenience
    /// Implements requirements 4.1 and 4.3: generate exactly 20 tracks and replace previous content
    pub async fn generate_and_replace_discovery_playlist(&self) -> DiscoveryResult<DiscoveryPlaylist> {
        // Generate the discovery playlist
        let discovery_playlist = self.generate_weekly_playlist().await?;
        
        // Extract track URIs for playlist replacement
        let track_uris: Vec<String> = discovery_playlist.tracks
            .iter()
            .map(|track| track.uri.clone())
            .collect();

        // Replace the discovery playlist tracks
        {
            let manager = self.playlist_manager.lock().await;
            manager.replace_discovery_playlist(track_uris).await
                .map_err(|e| DiscoveryError::PlaylistCreationFailed(
                    format!("Failed to replace discovery playlist: {:?}", e)
                ))?;
        }

        log::info!("Successfully generated and replaced discovery playlist with {} tracks", 
                  discovery_playlist.track_count());

        Ok(discovery_playlist)
    }

    /// Generate, replace, and announce the discovery playlist
    /// This is the complete workflow for weekly discovery playlist updates
    /// Implements requirements 4.1, 4.3, and 4.5: generate, replace, and announce
    pub async fn generate_and_announce_discovery_playlist(
        &self, 
        announcer: &crate::discord_announcer::DiscordAnnouncer
    ) -> DiscoveryResult<DiscoveryPlaylist> {
        // Generate and replace the discovery playlist
        let discovery_playlist = self.generate_and_replace_discovery_playlist().await?;
        
        // Announce the new discovery playlist
        if let Err(e) = announcer.announce_discovery_playlist(&discovery_playlist).await {
            log::error!("Failed to announce discovery playlist: {:?}", e);
            // Don't fail the entire operation if announcement fails
            // The playlist was still successfully generated and replaced
        }

        log::info!("Successfully completed discovery playlist generation and announcement workflow");
        Ok(discovery_playlist)
    }

    /// Get statistics about the current discovery generation capabilities
    pub async fn get_generation_stats(&self) -> DiscoveryResult<GenerationStats> {
        let collaborative_tracks = {
            let manager = self.playlist_manager.lock().await;
            manager.get_collaborative_tracks().await
                .map_err(|e| DiscoveryError::RecommendationGenerationFailed(
                    format!("Failed to get collaborative tracks: {:?}", e)
                ))?
        };

        let total_tracks = collaborative_tracks.len();
        let recent_pool_size = std::cmp::min(50, total_tracks);
        let max_seed_tracks = std::cmp::min(5, total_tracks);

        Ok(GenerationStats {
            total_collaborative_tracks: total_tracks,
            recent_tracks_pool_size: recent_pool_size,
            max_seed_tracks,
            can_generate: total_tracks > 0,
        })
    }
}

/// Statistics about discovery generation capabilities
#[derive(Debug, Clone)]
pub struct GenerationStats {
    /// Total number of tracks in collaborative playlist
    pub total_collaborative_tracks: usize,
    /// Size of recent tracks pool used for seed selection
    pub recent_tracks_pool_size: usize,
    /// Maximum number of seed tracks that can be selected
    pub max_seed_tracks: usize,
    /// Whether discovery generation is possible
    pub can_generate: bool,
}

impl GenerationStats {
    /// Format the statistics for display
    pub fn format_stats(&self) -> String {
        format!(
            "🎯 **Discovery Generation Stats**\n\
            • Total collaborative tracks: {}\n\
            • Recent tracks pool: {}\n\
            • Max seed tracks: {}\n\
            • Can generate: {}",
            self.total_collaborative_tracks,
            self.recent_tracks_pool_size,
            self.max_seed_tracks,
            if self.can_generate { "✅ Yes" } else { "❌ No" }
        )
    }
}