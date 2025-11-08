use log::{error, info};
use sonic::config::utils::load_config_with_details;
use sonic::spotify_client::SpotifyClient;

#[tokio::main]
async fn main() {
    // Load .env file if it exists
    let _ = dotenv::dotenv();
    
    // Initialize logging
    env_logger::init();
    
    info!("==============================================");
    info!("Spotify Recommendations API Test");
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

    // Test with a known-good track ID from Spotify's documentation
    info!("Testing recommendations API with Spotify's example track...");
    let test_seed = vec!["0c6xIDDpzE81m2q797ordA".to_string()]; // From Spotify docs
    
    match spotify_client.get_recommendations(test_seed).await {
        Ok(recommendations) => {
            info!("✅ Recommendations API works!");
            info!("   Got {} recommendations", recommendations.len());
            if !recommendations.is_empty() {
                info!("\n   Sample recommendations:");
                for (i, track) in recommendations.iter().take(3).enumerate() {
                    info!("   {}. {} - {}", i + 1, track.name, track.artists.join(", "));
                }
            }
        }
        Err(e) => {
            error!("❌ Recommendations API failed: {}", e);
            error!("\nPossible issues:");
            error!("- Your Spotify account may not have access to the recommendations API");
            error!("- The API might not be available in your region");
            error!("- Your refresh token might not have the required scopes");
            error!("\nThe recommendations API is part of Spotify's Web API and should be");
            error!("available to all developers, but there may be regional restrictions.");
        }
    }
}
