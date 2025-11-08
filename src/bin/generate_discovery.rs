use log::{error, info};
use sonic::config::utils::load_config_with_details;
use sonic::discovery_generator::DiscoveryGenerator;
use sonic::playlist_manager::PlaylistManager;
use sonic::spotify_client::SpotifyClient;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    // Load .env file if it exists
    let _ = dotenv::dotenv();
    
    // Initialize logging
    env_logger::init();
    
    info!("==============================================");
    info!("Manual Discovery Playlist Generator");
    info!("==============================================\n");

    // Load configuration
    let config = match load_config_with_details() {
        Ok(config) => {
            info!("✅ Configuration loaded successfully");
            config
        }
        Err(e) => {
            error!("❌ Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    info!("Collaborative Playlist ID: {}", config.collaborative_playlist_id);
    info!("Discovery Playlist ID: {}", config.discovery_playlist_id);
    info!("");

    // Initialize Spotify client
    info!("Initializing Spotify client...");
    let mut spotify_client = SpotifyClient::new(&config);
    
    if let Err(e) = spotify_client.initialize().await {
        error!("❌ Failed to initialize Spotify client: {}", e);
        std::process::exit(1);
    }
    info!("✅ Spotify client initialized\n");

    // Wrap in Arc<Mutex<>> for shared access
    let spotify_client = Arc::new(Mutex::new(spotify_client));
    
    // Initialize playlist manager
    let playlist_manager = Arc::new(Mutex::new(PlaylistManager::new(
        spotify_client.clone(),
        config.clone(),
    )));

    // Initialize discovery generator
    let discovery_generator = DiscoveryGenerator::new(
        spotify_client.clone(),
        playlist_manager.clone(),
        config.clone(),
    );

    // Generate discovery playlist
    info!("==============================================");
    info!("Generating Discovery Playlist");
    info!("==============================================\n");
    
    // Step 1: Get collaborative tracks
    info!("Step 1: Fetching tracks from collaborative playlist...");
    let collaborative_tracks = {
        let manager = playlist_manager.lock().await;
        match manager.get_collaborative_tracks().await {
            Ok(tracks) => {
                info!("✅ Found {} tracks in collaborative playlist", tracks.len());
                tracks
            }
            Err(e) => {
                error!("❌ Failed to get collaborative tracks: {}", e);
                std::process::exit(1);
            }
        }
    };

    // Step 2: Select seed tracks
    info!("\nStep 2: Selecting seed tracks...");
    let seed_tracks = match discovery_generator.select_seed_tracks(collaborative_tracks).await {
        Ok(seeds) => {
            info!("✅ Selected {} seed tracks", seeds.len());
            for (i, seed_id) in seeds.iter().enumerate() {
                info!("   Seed {}: {}", i + 1, seed_id);
            }
            seeds
        }
        Err(e) => {
            error!("❌ Failed to select seed tracks: {}", e);
            std::process::exit(1);
        }
    };

    // Step 2.5: Verify seed tracks are valid
    info!("\nStep 2.5: Verifying seed tracks are accessible...");
    {
        let mut client = spotify_client.lock().await;
        for (i, track_id) in seed_tracks.iter().enumerate() {
            match client.get_track_info(track_id).await {
                Ok(track) => {
                    info!("   ✅ Seed {} valid: {} - {}", i + 1, track.name, track.artists.join(", "));
                }
                Err(e) => {
                    error!("   ❌ Seed {} invalid ({}): {}", i + 1, track_id, e);
                }
            }
        }
    }

    // Step 3: Get recommendations
    info!("\nStep 3: Getting recommendations from Spotify...");
    let recommendations = match discovery_generator.get_recommendations(seed_tracks).await {
        Ok(recs) => {
            info!("✅ Got {} recommendations", recs.len());
            recs
        }
        Err(e) => {
            error!("❌ Failed to get recommendations: {}", e);
            error!("\nThis is where the 404 error is happening!");
            error!("The seed track IDs might be invalid or the recommendations endpoint is failing.");
            std::process::exit(1);
        }
    };

    // Step 4: Replace discovery playlist
    info!("\nStep 4: Replacing discovery playlist tracks...");
    let track_uris: Vec<String> = recommendations.iter().take(20).map(|t| t.uri.clone()).collect();
    info!("Replacing with {} tracks", track_uris.len());
    
    {
        let manager = playlist_manager.lock().await;
        match manager.replace_discovery_playlist(track_uris).await {
            Ok(_) => {
                info!("✅ Successfully replaced discovery playlist");
            }
            Err(e) => {
                error!("❌ Failed to replace discovery playlist: {}", e);
                error!("\nThis could be a permissions issue or invalid playlist ID.");
                std::process::exit(1);
            }
        }
    }

    info!("\n==============================================");
    info!("🎉 Success!");
    info!("==============================================");
    info!("Discovery playlist has been updated with {} new tracks!", recommendations.len().min(20));
}
