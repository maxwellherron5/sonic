use crate::error::{MessageProcessingError, MessageProcessingResult};
use crate::models::SpotifyUrlType;
use log::{debug, warn};
use regex::Regex;
use url::Url;

/// Message processor for extracting and validating Spotify URLs from Discord messages
pub struct MessageProcessor {
    /// Regex for matching Spotify URLs in text
    spotify_url_regex: Regex,
    /// Regex for matching Spotify URIs
    spotify_uri_regex: Regex,
}

impl MessageProcessor {
    /// Create a new MessageProcessor instance
    pub fn new() -> Self {
        // Regex to match Spotify URLs (both HTTP and HTTPS)
        let spotify_url_regex = Regex::new(
            r"https?://(?:open\.)?spotify\.com/(?:intl-[a-z]{2}/)?(?:user/[^/]+/)?(?:track|album|playlist|artist)/([a-zA-Z0-9]+)(?:\?[^\s]*)?",
        ).expect("Failed to compile Spotify URL regex");

        // Regex to match Spotify URIs (spotify:track:id format)
        let spotify_uri_regex = Regex::new(
            r"spotify:(track|album|playlist|artist):([a-zA-Z0-9]+)",
        ).expect("Failed to compile Spotify URI regex");

        Self {
            spotify_url_regex,
            spotify_uri_regex,
        }
    }

    /// Extract all Spotify URLs from a message content
    pub fn extract_spotify_urls(&self, content: &str) -> Vec<String> {
        let mut urls = Vec::new();

        // Find HTTP/HTTPS URLs
        for capture in self.spotify_url_regex.captures_iter(content) {
            if let Some(full_match) = capture.get(0) {
                urls.push(full_match.as_str().to_string());
            }
        }

        // Find Spotify URIs
        for capture in self.spotify_uri_regex.captures_iter(content) {
            if let Some(full_match) = capture.get(0) {
                urls.push(full_match.as_str().to_string());
            }
        }

        // Also check for URLs that might be split by whitespace or other characters
        let words: Vec<&str> = content.split_whitespace().collect();
        for word in words {
            if self.is_potential_spotify_url(word) && !urls.contains(&word.to_string()) {
                // Try to clean up the URL (remove trailing punctuation, etc.)
                let cleaned_url = self.clean_url(word);
                if self.is_valid_spotify_url_format(&cleaned_url) {
                    urls.push(cleaned_url);
                }
            }
        }

        debug!("Extracted {} Spotify URLs from message", urls.len());
        urls
    }

    /// Parse a Spotify URL and determine its type and ID
    pub fn parse_spotify_url(&self, url: &str) -> MessageProcessingResult<SpotifyUrlType> {
        debug!("Parsing Spotify URL: {}", url);

        // Handle Spotify URIs (spotify:track:id format)
        if url.starts_with("spotify:") {
            return self.parse_spotify_uri(url);
        }

        // Handle HTTP/HTTPS URLs
        let parsed_url = Url::parse(url)
            .map_err(|e| {
                warn!("Failed to parse URL '{}': {}", url, e);
                MessageProcessingError::UrlParsingFailed(format!("Invalid URL format: {}", e))
            })?;

        // Validate that it's a Spotify URL
        let host = parsed_url.host_str().ok_or_else(|| {
            MessageProcessingError::InvalidSpotifyUrl {
                url: url.to_string(),
            }
        })?;

        if !self.is_spotify_host(host) {
            return Err(MessageProcessingError::InvalidSpotifyUrl {
                url: url.to_string(),
            });
        }

        // Parse the path to extract content type and ID
        let path = parsed_url.path();
        self.parse_spotify_path(path, url)
    }

    /// Extract track ID from a Spotify URL (only works for track URLs)
    pub fn extract_track_id(&self, url: &str) -> MessageProcessingResult<String> {
        match self.parse_spotify_url(url)? {
            SpotifyUrlType::Track(id) => {
                debug!("Extracted track ID '{}' from URL '{}'", id, url);
                Ok(id)
            }
            other_type => {
                warn!("URL '{}' is not a track URL, found: {:?}", url, other_type);
                Err(MessageProcessingError::TrackIdExtractionFailed {
                    url: url.to_string(),
                })
            }
        }
    }

    /// Validate that a URL is a supported Spotify track URL
    pub fn validate_track_url(&self, url: &str) -> MessageProcessingResult<String> {
        match self.parse_spotify_url(url)? {
            SpotifyUrlType::Track(id) => Ok(id),
            SpotifyUrlType::Unsupported => Err(MessageProcessingError::UnsupportedUrlType {
                url: url.to_string(),
            }),
            _ => Err(MessageProcessingError::UnsupportedUrlType {
                url: url.to_string(),
            }),
        }
    }

    /// Check if a string might contain a Spotify URL
    fn is_potential_spotify_url(&self, text: &str) -> bool {
        text.contains("spotify.com") || text.starts_with("spotify:")
    }

    /// Check if a host is a valid Spotify host
    fn is_spotify_host(&self, host: &str) -> bool {
        matches!(host, "open.spotify.com" | "spotify.com")
    }

    /// Check if a URL has a valid Spotify URL format
    fn is_valid_spotify_url_format(&self, url: &str) -> bool {
        self.spotify_url_regex.is_match(url) || self.spotify_uri_regex.is_match(url)
    }

    /// Clean up a URL by removing trailing punctuation and other artifacts
    fn clean_url(&self, url: &str) -> String {
        // Remove common trailing punctuation that might be included in URLs
        let mut cleaned = url.trim_end_matches(&['.', ',', '!', '?', ')', ']', '}', ';', ':'][..]);
        
        // Remove leading punctuation
        cleaned = cleaned.trim_start_matches(&['(', '[', '{'][..]);
        
        cleaned.to_string()
    }

    /// Parse a Spotify URI (spotify:track:id format)
    fn parse_spotify_uri(&self, uri: &str) -> MessageProcessingResult<SpotifyUrlType> {
        let parts: Vec<&str> = uri.split(':').collect();
        
        if parts.len() != 3 || parts[0] != "spotify" {
            return Err(MessageProcessingError::InvalidSpotifyUrl {
                url: uri.to_string(),
            });
        }

        let content_type = parts[1];
        let id = parts[2];

        // Validate ID format (should be alphanumeric)
        if !id.chars().all(|c| c.is_alphanumeric()) {
            return Err(MessageProcessingError::InvalidSpotifyUrl {
                url: uri.to_string(),
            });
        }

        match content_type {
            "track" => Ok(SpotifyUrlType::Track(id.to_string())),
            "album" => Ok(SpotifyUrlType::Album(id.to_string())),
            "playlist" => Ok(SpotifyUrlType::Playlist(id.to_string())),
            "artist" => Ok(SpotifyUrlType::Artist(id.to_string())),
            _ => Ok(SpotifyUrlType::Unsupported),
        }
    }

    /// Parse a Spotify URL path to extract content type and ID
    fn parse_spotify_path(&self, path: &str, original_url: &str) -> MessageProcessingResult<SpotifyUrlType> {
        let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        // Handle different path formats:
        // /track/id
        // /intl-xx/track/id
        // /user/username/playlist/id (legacy format)
        
        let (content_type, id) = if path_segments.len() >= 2 {
            // Standard format: /track/id or /intl-xx/track/id
            if path_segments.len() == 2 {
                (path_segments[0], path_segments[1])
            } else if path_segments.len() >= 3 && path_segments[0].starts_with("intl-") {
                // International format: /intl-xx/track/id
                (path_segments[1], path_segments[2])
            } else if path_segments.len() >= 4 && path_segments[0] == "user" {
                // Legacy user playlist format: /user/username/playlist/id
                (path_segments[2], path_segments[3])
            } else {
                // Try the last two segments
                let len = path_segments.len();
                (path_segments[len - 2], path_segments[len - 1])
            }
        } else {
            return Err(MessageProcessingError::InvalidSpotifyUrl {
                url: original_url.to_string(),
            });
        };

        // Clean the ID (remove query parameters if they somehow got included)
        let clean_id = id.split('?').next().unwrap_or(id);

        // Validate ID format
        if !clean_id.chars().all(|c| c.is_alphanumeric()) {
            return Err(MessageProcessingError::InvalidSpotifyUrl {
                url: original_url.to_string(),
            });
        }

        match content_type {
            "track" => Ok(SpotifyUrlType::Track(clean_id.to_string())),
            "album" => Ok(SpotifyUrlType::Album(clean_id.to_string())),
            "playlist" => Ok(SpotifyUrlType::Playlist(clean_id.to_string())),
            "artist" => Ok(SpotifyUrlType::Artist(clean_id.to_string())),
            _ => Ok(SpotifyUrlType::Unsupported),
        }
    }
}

impl Default for MessageProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for message processing operations
pub trait MessageProcessing {
    /// Extract Spotify URLs from message content
    fn extract_spotify_urls(&self, content: &str) -> Vec<String>;
    
    /// Parse a Spotify URL and determine its type
    fn parse_spotify_url(&self, url: &str) -> MessageProcessingResult<SpotifyUrlType>;
    
    /// Extract track ID from a Spotify URL
    fn extract_track_id(&self, url: &str) -> MessageProcessingResult<String>;
    
    /// Validate that a URL is a supported track URL
    fn validate_track_url(&self, url: &str) -> MessageProcessingResult<String>;
}

impl MessageProcessing for MessageProcessor {
    fn extract_spotify_urls(&self, content: &str) -> Vec<String> {
        self.extract_spotify_urls(content)
    }

    fn parse_spotify_url(&self, url: &str) -> MessageProcessingResult<SpotifyUrlType> {
        self.parse_spotify_url(url)
    }

    fn extract_track_id(&self, url: &str) -> MessageProcessingResult<String> {
        self.extract_track_id(url)
    }

    fn validate_track_url(&self, url: &str) -> MessageProcessingResult<String> {
        self.validate_track_url(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_spotify_urls() {
        let processor = MessageProcessor::new();
        
        // Test with various message formats
        let test_cases = vec![
            (
                "Check out this song: https://open.spotify.com/track/4iV5W9uYEdYUVa79Axb7Rh",
                vec!["https://open.spotify.com/track/4iV5W9uYEdYUVa79Axb7Rh"],
            ),
            (
                "Multiple songs: https://open.spotify.com/track/1 and https://open.spotify.com/track/2",
                vec!["https://open.spotify.com/track/1", "https://open.spotify.com/track/2"],
            ),
            (
                "Spotify URI: spotify:track:4iV5W9uYEdYUVa79Axb7Rh",
                vec!["spotify:track:4iV5W9uYEdYUVa79Axb7Rh"],
            ),
            (
                "No Spotify URLs here",
                vec![],
            ),
        ];

        for (input, expected) in test_cases {
            let result = processor.extract_spotify_urls(input);
            assert_eq!(result, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_parse_spotify_url() {
        let processor = MessageProcessor::new();

        // Test track URL
        let track_url = "https://open.spotify.com/track/4iV5W9uYEdYUVa79Axb7Rh";
        match processor.parse_spotify_url(track_url).unwrap() {
            SpotifyUrlType::Track(id) => assert_eq!(id, "4iV5W9uYEdYUVa79Axb7Rh"),
            _ => panic!("Expected track type"),
        }

        // Test Spotify URI
        let track_uri = "spotify:track:4iV5W9uYEdYUVa79Axb7Rh";
        match processor.parse_spotify_url(track_uri).unwrap() {
            SpotifyUrlType::Track(id) => assert_eq!(id, "4iV5W9uYEdYUVa79Axb7Rh"),
            _ => panic!("Expected track type"),
        }

        // Test invalid URL
        let invalid_url = "https://example.com/not-spotify";
        assert!(processor.parse_spotify_url(invalid_url).is_err());
    }

    #[test]
    fn test_extract_track_id() {
        let processor = MessageProcessor::new();

        // Test valid track URL
        let track_url = "https://open.spotify.com/track/4iV5W9uYEdYUVa79Axb7Rh";
        let track_id = processor.extract_track_id(track_url).unwrap();
        assert_eq!(track_id, "4iV5W9uYEdYUVa79Axb7Rh");

        // Test non-track URL
        let album_url = "https://open.spotify.com/album/4iV5W9uYEdYUVa79Axb7Rh";
        assert!(processor.extract_track_id(album_url).is_err());
    }

    #[test]
    fn test_validate_track_url() {
        let processor = MessageProcessor::new();

        // Test valid track URL
        let track_url = "https://open.spotify.com/track/4iV5W9uYEdYUVa79Axb7Rh";
        let track_id = processor.validate_track_url(track_url).unwrap();
        assert_eq!(track_id, "4iV5W9uYEdYUVa79Axb7Rh");

        // Test unsupported URL type
        let album_url = "https://open.spotify.com/album/4iV5W9uYEdYUVa79Axb7Rh";
        assert!(processor.validate_track_url(album_url).is_err());
    }
}