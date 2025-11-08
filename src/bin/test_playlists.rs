use log::{error, info, warn};
use sonic::config::utils::load_config_with_details;
use sonic::spotify_client::SpotifyClient;

#[tokio::main]
async fn main() {
    // Load .env file if it exists
    let _ = dotenv::dotenv();
    
    // Initialize logging
    env_logger::init();
    
    info!("==============================================");
    info!("Playlist Diagnostic Tool");
    info!("==============================================\n");

    // Load configuration
    let config = match load_config_with_details() {
        Ok(config) => config,
        Err(e) => {
            error!("❌ Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize Spotify client
    info!("Initializing Spotify client...");
    let mut spotify_client = SpotifyClient::new(&config);
    
    if let Err(e) = spotify_client.initialize().await {
        error!("❌ Failed to initialize Spotify client: {}", e);
        std::process::exit(1);
    }
    info!("✅ Spotify client initialized\n");

    // Test collaborative playlist
    info!("==============================================");
    info!("Testing Collaborative Playlist");
    info!("==============================================");
    info!("Playlist ID: {}\n", config.collaborative_playlist_id);
    
    match spotify_client.get_playlist_tracks(&config.collaborative_playlist_id).await {
        Ok(tracks) => {
            info!("✅ Collaborative playlist is accessible!");
            info!("   Track count: {}", tracks.len());
            
            if tracks.is_empty() {
                warn!("⚠️  Playlist is empty - you need to add tracks before generating discovery playlists");
            } else {
                info!("\n   Sample tracks:");
                for (i, track) in tracks.iter().take(5).enumerate() {
                    info!("   {}. {} - {}", i + 1, track.name, track.artists.join(", "));
                }
                if tracks.len() > 5 {
                    info!("   ... and {} more", tracks.len() - 5);
                }
            }
        }
        Err(e) => {
            error!("❌ Cannot access collaborative playlist: {}", e);
            error!("\n   Possible issues:");
            error!("   - Playlist ID is incorrect");
            error!("   - Playlist doesn't exist");
            error!("   - Playlist is private and not owned by your Spotify account");
            error!("\n   To fix:");
            error!("   1. Go to the playlist in Spotify");
            error!("   2. Click '...' → Share → Copy link to playlist");
            error!("   3. Extract the ID from: https://open.spotify.com/playlist/ID_HERE");
            error!("   4. Update COLLABORATIVE_PLAYLIST_ID in your .env file");
        }
    }

    // Test discovery playlist
    info!("\n==============================================");
    info!("Testing Discovery Playlist");
    info!("==============================================");
    info!("Playlist ID: {}\n", config.discovery_playlist_id);
    
    match spotify_client.get_playlist_tracks(&config.discovery_playlist_id).await {
        Ok(tracks) => {
            info!("✅ Discovery playlist is accessible!");
            info!("   Track count: {}", tracks.len());
            
            if !tracks.is_empty() {
                info!("\n   Current tracks:");
                for (i, track) in tracks.iter().take(5).enumerate() {
                    info!("   {}. {} - {}", i + 1, track.name, track.artists.join(", "));
                }
                if tracks.len() > 5 {
                    info!("   ... and {} more", tracks.len() - 5);
                }
            }
        }
        Err(e) => {
            error!("❌ Cannot access discovery playlist: {}", e);
            error!("\n   Possible issues:");
            error!("   - Playlist ID is incorrect");
            error!("   - Playlist doesn't exist");
            error!("   - Playlist is private and not owned by your Spotify account");
            error!("\n   To fix:");
            error!("   1. Go to the playlist in Spotify");
            error!("   2. Click '...' → Share → Copy link to playlist");
            error!("   3. Extract the ID from: https://open.spotify.com/playlist/ID_HERE");
            error!("   4. Update DISCOVERY_PLAYLIST_ID in your .env file");
        }
    }

    info!("\n==============================================");
    info!("Diagnostic Complete");
    info!("==============================================");
}
