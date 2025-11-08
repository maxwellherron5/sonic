use log::{error, info, warn};
use sonic::config::utils::load_config_with_details;
use sonic::spotify_client::SpotifyClient;
use sonic::message_processor::MessageProcessor;
use sonic::models::SpotifyUrlType;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::main]
async fn main() {
    // Load .env file if it exists
    let _ = dotenv::dotenv();
    
    // Initialize logging
    env_logger::init();
    
    info!("Starting Discord Spotify Bot Integration Test");
    info!("==============================================");

    // Test 1: Configuration Loading
    info!("Test 1: Loading and validating configuration...");
    let config = match load_config_with_details() {
        Ok(config) => {
            info!("✅ Configuration loaded successfully");
            info!("   - Discord token: {}***", &config.discord_token[..8]);
            info!("   - Target channel ID: {}", config.target_channel_id);
            info!("   - Spotify client ID: {}", config.spotify_client_id);
            info!("   - Collaborative playlist: {}", config.collaborative_playlist_id);
            info!("   - Discovery playlist: {}", config.discovery_playlist_id);
            config
        }
        Err(e) => {
            error!("❌ Configuration loading failed: {}", e);
            std::process::exit(1);
        }
    };

    // Test 2: Message Processing
    info!("\nTest 2: Testing message processing...");
    test_message_processing().await;

    // Test 3: Spotify API Connectivity
    info!("\nTest 3: Testing Spotify API connectivity...");
    test_spotify_connectivity(&config).await;

    // Test 4: Playlist Operations
    info!("\nTest 4: Testing playlist operations...");
    test_playlist_operations(&config).await;

    info!("\n==============================================");
    info!("Integration test completed!");
    info!("If all tests passed, your bot should be ready to run.");
}

async fn test_message_processing() {
    let processor = MessageProcessor::new();
    
    // Test various Spotify URL formats
    let test_urls = vec![
        "https://open.spotify.com/track/4iV5W9uYEdYUVa79Axb7Rh",
        "https://open.spotify.com/track/4iV5W9uYEdYUVa79Axb7Rh?si=abc123",
        "spotify:track:4iV5W9uYEdYUVa79Axb7Rh",
        "Check out this song: https://open.spotify.com/track/4iV5W9uYEdYUVa79Axb7Rh",
        "https://open.spotify.com/album/1DFixLWuPkv3KT3TnV35m3",
        "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
        "Not a Spotify URL: https://youtube.com/watch?v=abc123",
    ];

    let mut passed = 0;
    let mut failed = 0;

    for (i, test_message) in test_urls.iter().enumerate() {
        info!("   Testing URL {}: {}", i + 1, test_message);
        
        let urls = processor.extract_spotify_urls(test_message);
        if urls.is_empty() && test_message.contains("spotify") {
            warn!("     ⚠️  No URLs extracted from Spotify message");
            failed += 1;
            continue;
        }

        for url in urls {
            match processor.parse_spotify_url(&url) {
                Ok(SpotifyUrlType::Track(id)) => {
                    info!("     ✅ Track ID extracted: {}", id);
                    passed += 1;
                }
                Ok(SpotifyUrlType::Album(id)) => {
                    info!("     ✅ Album ID extracted: {}", id);
                    passed += 1;
                }
                Ok(SpotifyUrlType::Playlist(id)) => {
                    info!("     ✅ Playlist ID extracted: {}", id);
                    passed += 1;
                }
                Ok(SpotifyUrlType::Unsupported) => {
                    info!("     ℹ️  Unsupported Spotify URL type");
                    passed += 1;
                }
                Ok(SpotifyUrlType::Artist(id)) => {
                    info!("     ✅ Artist ID extracted: {}", id);
                    passed += 1;
                }
                Err(e) => {
                    if test_message.contains("youtube") {
                        info!("     ✅ Correctly rejected non-Spotify URL");
                        passed += 1;
                    } else {
                        error!("     ❌ URL parsing failed: {}", e);
                        failed += 1;
                    }
                }
            }
        }
    }

    info!("Message processing test results: {} passed, {} failed", passed, failed);
}

async fn test_spotify_connectivity(config: &sonic::models::BotConfig) {
    let mut spotify_client = SpotifyClient::new(config);

    // Test authentication
    info!("   Testing Spotify authentication...");
    match timeout(Duration::from_secs(10), spotify_client.initialize()).await {
        Ok(Ok(())) => {
            info!("   ✅ Spotify authentication successful");
        }
        Ok(Err(e)) => {
            error!("   ❌ Spotify authentication failed: {}", e);
            return;
        }
        Err(_) => {
            error!("   ❌ Spotify authentication timed out");
            return;
        }
    }

    // Test basic API connectivity by getting a test track
    info!("   Testing basic API connectivity...");
    match timeout(Duration::from_secs(10), spotify_client.get_track_info("4iV5W9uYEdYUVa79Axb7Rh")).await {
        Ok(Ok(track)) => {
            info!("   ✅ Connected to Spotify API successfully - test track: {}", track.name);
        }
        Ok(Err(e)) => {
            error!("   ❌ Failed to get track info: {}", e);
        }
        Err(_) => {
            error!("   ❌ API request timed out");
        }
    }
}

async fn test_playlist_operations(config: &sonic::models::BotConfig) {
    let mut spotify_client = SpotifyClient::new(config);

    // Ensure we're authenticated
    if let Err(e) = spotify_client.initialize().await {
        error!("   ❌ Cannot test playlists - authentication failed: {}", e);
        return;
    }

    // Test collaborative playlist access
    info!("   Testing collaborative playlist access...");
    match timeout(
        Duration::from_secs(15),
        spotify_client.get_playlist_tracks(&config.collaborative_playlist_id)
    ).await {
        Ok(Ok(tracks)) => {
            info!("   ✅ Collaborative playlist accessible ({} tracks found)", tracks.len());
        }
        Ok(Err(e)) => {
            error!("   ❌ Cannot access collaborative playlist: {}", e);
            error!("      Check that playlist ID is correct and playlist is public/owned by authenticated user");
        }
        Err(_) => {
            error!("   ❌ Playlist access timed out");
        }
    }

    // Test discovery playlist access
    info!("   Testing discovery playlist access...");
    match timeout(
        Duration::from_secs(15),
        spotify_client.get_playlist_tracks(&config.discovery_playlist_id)
    ).await {
        Ok(Ok(tracks)) => {
            info!("   ✅ Discovery playlist accessible ({} tracks found)", tracks.len());
        }
        Ok(Err(e)) => {
            error!("   ❌ Cannot access discovery playlist: {}", e);
            error!("      Check that playlist ID is correct and playlist is public/owned by authenticated user");
        }
        Err(_) => {
            error!("   ❌ Playlist access timed out");
        }
    }

    // Test recommendations API (if collaborative playlist has tracks)
    info!("   Testing Spotify recommendations API...");
    match timeout(
        Duration::from_secs(15),
        spotify_client.get_playlist_tracks(&config.collaborative_playlist_id)
    ).await {
        Ok(Ok(tracks)) if !tracks.is_empty() => {
            let seed_tracks: Vec<String> = tracks.into_iter()
                .take(2)
                .map(|t| t.id)
                .collect();
            
            match timeout(
                Duration::from_secs(15),
                spotify_client.get_recommendations(seed_tracks)
            ).await {
                Ok(Ok(recommendations)) => {
                    info!("   ✅ Recommendations API working ({} recommendations received)", recommendations.len());
                }
                Ok(Err(e)) => {
                    error!("   ❌ Recommendations API failed: {}", e);
                }
                Err(_) => {
                    error!("   ❌ Recommendations API timed out");
                }
            }
        }
        Ok(Ok(_)) => {
            info!("   ℹ️  Collaborative playlist is empty - skipping recommendations test");
        }
        Ok(Err(e)) => {
            error!("   ❌ Cannot test recommendations - playlist access failed: {}", e);
        }
        Err(_) => {
            error!("   ❌ Cannot test recommendations - playlist access timed out");
        }
    }
}