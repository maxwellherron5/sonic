use log::{debug, warn};
use std::time::Duration;
use tokio::time::sleep;

/// Retry configuration for API calls
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Base delay between retries in milliseconds
    pub base_delay_ms: u64,
    /// Maximum delay between retries in milliseconds
    pub max_delay_ms: u64,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Jitter factor (0.0 to 1.0) to add randomness to delays
    pub jitter_factor: f64,
}

impl RetryConfig {
    /// Create a new retry configuration with default values
    pub fn new() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.25,
        }
    }

    /// Create a retry configuration from bot config values
    pub fn from_bot_config(max_attempts: u32, base_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            max_attempts,
            base_delay_ms,
            max_delay_ms,
            backoff_multiplier: 2.0,
            jitter_factor: 0.25,
        }
    }

    /// Calculate the delay for a given attempt number
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let base_delay = self.base_delay_ms as f64;
        let exponential_delay = base_delay * self.backoff_multiplier.powi(attempt as i32);
        
        // Apply jitter
        let jitter = if self.jitter_factor > 0.0 {
            let jitter_range = exponential_delay * self.jitter_factor;
            let random_jitter = (rand::random::<f64>() - 0.5) * 2.0 * jitter_range;
            random_jitter
        } else {
            0.0
        };

        let final_delay = (exponential_delay + jitter).min(self.max_delay_ms as f64);
        Duration::from_millis(final_delay.max(0.0) as u64)
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a function with retry logic and exponential backoff
pub async fn retry_with_backoff<F, Fut, T, E>(
    operation: F,
    config: &RetryConfig,
    operation_name: &str,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut last_error = None;

    for attempt in 0..config.max_attempts {
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    debug!("{} succeeded on attempt {}", operation_name, attempt + 1);
                }
                return Ok(result);
            }
            Err(error) => {
                warn!(
                    "{} failed on attempt {} of {}: {}",
                    operation_name,
                    attempt + 1,
                    config.max_attempts,
                    error
                );

                last_error = Some(error);

                // Don't sleep after the last attempt
                if attempt < config.max_attempts - 1 {
                    let delay = config.calculate_delay(attempt);
                    debug!("Retrying {} in {:?}", operation_name, delay);
                    sleep(delay).await;
                }
            }
        }
    }

    // Return the last error if all attempts failed
    Err(last_error.unwrap())
}

/// Check if an error is retryable (e.g., network errors, rate limits)
pub fn is_retryable_error<E: std::fmt::Display>(error: &E) -> bool {
    let error_str = error.to_string().to_lowercase();
    
    // Common retryable error patterns
    error_str.contains("timeout") ||
    error_str.contains("connection") ||
    error_str.contains("network") ||
    error_str.contains("rate limit") ||
    error_str.contains("429") ||
    error_str.contains("502") ||
    error_str.contains("503") ||
    error_str.contains("504")
}

/// Utility functions for working with Spotify URLs
pub mod spotify_url {
    use crate::error::{MessageProcessingError, MessageProcessingResult};
    use crate::models::SpotifyUrlType;
    use url::Url;

    /// Extract Spotify URLs from a text message
    pub fn extract_spotify_urls(content: &str) -> Vec<String> {
        let words: Vec<&str> = content.split_whitespace().collect();
        let mut urls = Vec::new();

        for word in words {
            if is_spotify_url(word) {
                urls.push(word.to_string());
            }
        }

        urls
    }

    /// Check if a string is a Spotify URL
    pub fn is_spotify_url(text: &str) -> bool {
        text.contains("spotify.com") || text.starts_with("spotify:")
    }

    /// Parse a Spotify URL and determine its type
    pub fn parse_spotify_url(url: &str) -> MessageProcessingResult<SpotifyUrlType> {
        // Handle spotify: URIs
        if url.starts_with("spotify:") {
            return parse_spotify_uri(url);
        }

        // Handle HTTP/HTTPS URLs
        let parsed_url = Url::parse(url)
            .map_err(|_| MessageProcessingError::UrlParsingFailed(url.to_string()))?;

        if parsed_url.host_str() != Some("open.spotify.com") {
            return Err(MessageProcessingError::InvalidSpotifyUrl {
                url: url.to_string(),
            });
        }

        let path_segments: Vec<&str> = parsed_url.path_segments()
            .ok_or_else(|| MessageProcessingError::InvalidSpotifyUrl {
                url: url.to_string(),
            })?
            .collect();

        if path_segments.len() < 2 {
            return Err(MessageProcessingError::InvalidSpotifyUrl {
                url: url.to_string(),
            });
        }

        let content_type = path_segments[0];
        let id = path_segments[1].to_string();

        match content_type {
            "track" => Ok(SpotifyUrlType::Track(id)),
            "album" => Ok(SpotifyUrlType::Album(id)),
            "playlist" => Ok(SpotifyUrlType::Playlist(id)),
            "artist" => Ok(SpotifyUrlType::Artist(id)),
            _ => Ok(SpotifyUrlType::Unsupported),
        }
    }

    /// Parse a Spotify URI (spotify:track:id format)
    fn parse_spotify_uri(uri: &str) -> MessageProcessingResult<SpotifyUrlType> {
        let parts: Vec<&str> = uri.split(':').collect();
        
        if parts.len() != 3 || parts[0] != "spotify" {
            return Err(MessageProcessingError::InvalidSpotifyUrl {
                url: uri.to_string(),
            });
        }

        let content_type = parts[1];
        let id = parts[2].to_string();

        match content_type {
            "track" => Ok(SpotifyUrlType::Track(id)),
            "album" => Ok(SpotifyUrlType::Album(id)),
            "playlist" => Ok(SpotifyUrlType::Playlist(id)),
            "artist" => Ok(SpotifyUrlType::Artist(id)),
            _ => Ok(SpotifyUrlType::Unsupported),
        }
    }

    /// Extract track ID from a Spotify URL or URI
    pub fn extract_track_id(url: &str) -> MessageProcessingResult<String> {
        match parse_spotify_url(url)? {
            SpotifyUrlType::Track(id) => Ok(id),
            _ => Err(MessageProcessingError::TrackIdExtractionFailed {
                url: url.to_string(),
            }),
        }
    }

    /// Convert a track ID to a Spotify URI
    pub fn track_id_to_uri(track_id: &str) -> String {
        format!("spotify:track:{}", track_id)
    }

    /// Validate that a URL is a supported Spotify track URL
    pub fn validate_track_url(url: &str) -> MessageProcessingResult<String> {
        match parse_spotify_url(url)? {
            SpotifyUrlType::Track(id) => Ok(id),
            SpotifyUrlType::Unsupported => Err(MessageProcessingError::UnsupportedUrlType {
                url: url.to_string(),
            }),
            _ => Err(MessageProcessingError::UnsupportedUrlType {
                url: url.to_string(),
            }),
        }
    }
}

/// Utility functions for logging and monitoring
pub mod logging {
    use log::{error, info, warn};
    use std::time::SystemTime;

    /// Log a successful track addition
    pub fn log_track_added(track_name: &str, artist: &str, user_id: u64, channel_id: u64) {
        info!(
            "Track added: '{}' by '{}' | User: {} | Channel: {} | Time: {:?}",
            track_name,
            artist,
            user_id,
            channel_id,
            SystemTime::now()
        );
    }

    /// Log a failed track addition
    pub fn log_track_add_failed(url: &str, error: &str, user_id: u64, channel_id: u64) {
        error!(
            "Failed to add track: {} | Error: {} | User: {} | Channel: {} | Time: {:?}",
            url,
            error,
            user_id,
            channel_id,
            SystemTime::now()
        );
    }

    /// Log API errors with context
    pub fn log_api_error(api_name: &str, endpoint: &str, status: u16, error: &str) {
        error!(
            "API Error: {} | Endpoint: {} | Status: {} | Error: {} | Time: {:?}",
            api_name,
            endpoint,
            status,
            error,
            SystemTime::now()
        );
    }

    /// Log retry attempts
    pub fn log_retry_attempt(operation: &str, attempt: u32, max_attempts: u32, error: &str) {
        warn!(
            "Retry attempt: {} | Attempt: {}/{} | Error: {} | Time: {:?}",
            operation,
            attempt,
            max_attempts,
            error,
            SystemTime::now()
        );
    }

    /// Log discovery playlist generation
    pub fn log_discovery_generated(track_count: usize, seed_count: usize) {
        info!(
            "Discovery playlist generated: {} tracks | {} seed tracks | Time: {:?}",
            track_count,
            seed_count,
            SystemTime::now()
        );
    }
}