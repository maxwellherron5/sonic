use thiserror::Error;

/// Main error type for the Discord Spotify Bot
#[derive(Debug, Clone, Error)]
pub enum BotError {
    #[error("Discord error: {0}")]
    Discord(#[from] DiscordError),
    #[error("Spotify error: {0}")]
    Spotify(#[from] SpotifyError),
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    #[error("Playlist error: {0}")]
    Playlist(#[from] PlaylistError),
    #[error("Message processing error: {0}")]
    MessageProcessing(#[from] MessageProcessingError),
    #[error("Discovery generation error: {0}")]
    Discovery(#[from] DiscoveryError),
    #[error("Scheduler error: {0}")]
    Scheduler(#[from] SchedulerError),
}

/// Discord-related errors
#[derive(Debug, Clone, Error)]
pub enum DiscordError {
    #[error("Failed to connect to Discord API: {0}")]
    ConnectionFailed(String),
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("Message send failed: {0}")]
    MessageSendFailed(String),
    #[error("Channel not found: {channel_id}")]
    ChannelNotFound { channel_id: u64 },
    #[error("Permission denied for channel: {channel_id}")]
    PermissionDenied { channel_id: u64 },
    #[error("Rate limit exceeded, retry after: {retry_after_ms}ms")]
    RateLimitExceeded { retry_after_ms: u64 },
    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },
}

/// Spotify-related errors
#[derive(Debug, Clone, Error)]
pub enum SpotifyError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("Token expired")]
    TokenExpired,
    #[error("Token refresh failed: {0}")]
    TokenRefreshFailed(String),
    #[error("API request failed: {status} - {message}")]
    ApiRequestFailed { status: u16, message: String },
    #[error("Rate limit exceeded, retry after: {retry_after_ms}ms")]
    RateLimitExceeded { retry_after_ms: u64 },
    #[error("Track not found: {track_id}")]
    TrackNotFound { track_id: String },
    #[error("Playlist not found: {playlist_id}")]
    PlaylistNotFound { playlist_id: String },
    #[error("Playlist access denied: {playlist_id}")]
    PlaylistAccessDenied { playlist_id: String },
    #[error("Invalid track URI: {uri}")]
    InvalidTrackUri { uri: String },
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("JSON parsing error: {0}")]
    JsonParsingError(String),
}

/// Configuration-related errors
#[derive(Debug, Clone, Error)]
pub enum ConfigError {
    #[error("Missing environment variable: {var_name}")]
    MissingEnvironmentVariable { var_name: String },
    #[error("Invalid configuration value for {field}: {value}")]
    InvalidValue { field: String, value: String },
    #[error("Configuration validation failed: {0}")]
    ValidationFailed(String),
    #[error("Failed to load configuration: {0}")]
    LoadFailed(String),
    #[error("Failed to save configuration: {0}")]
    SaveFailed(String),
}

/// Playlist operation errors
#[derive(Debug, Clone, Error)]
pub enum PlaylistError {
    #[error("Failed to add track to playlist: {0}")]
    AddTrackFailed(String),
    #[error("Failed to remove track from playlist: {0}")]
    RemoveTrackFailed(String),
    #[error("Failed to retrieve playlist tracks: {0}")]
    RetrieveTracksFailed(String),
    #[error("Track already exists in playlist: {track_uri}")]
    TrackAlreadyExists { track_uri: String },
    #[error("Playlist is full, cannot add more tracks")]
    PlaylistFull,
    #[error("Failed to replace playlist tracks: {0}")]
    ReplaceTracksFailed(String),
}

/// Message processing errors
#[derive(Debug, Clone, Error)]
pub enum MessageProcessingError {
    #[error("Invalid Spotify URL: {url}")]
    InvalidSpotifyUrl { url: String },
    #[error("Unsupported Spotify URL type: {url}")]
    UnsupportedUrlType { url: String },
    #[error("Failed to extract track ID from URL: {url}")]
    TrackIdExtractionFailed { url: String },
    #[error("URL parsing failed: {0}")]
    UrlParsingFailed(String),
}

/// Discovery playlist generation errors
#[derive(Debug, Clone, Error)]
pub enum DiscoveryError {
    #[error("Failed to generate recommendations: {0}")]
    RecommendationGenerationFailed(String),
    #[error("Insufficient seed tracks: found {count}, need at least {required}")]
    InsufficientSeedTracks { count: usize, required: usize },
    #[error("Failed to select seed tracks: {0}")]
    SeedSelectionFailed(String),
    #[error("Failed to create discovery playlist: {0}")]
    PlaylistCreationFailed(String),
}

/// Scheduler-related errors
#[derive(Debug, Clone, Error)]
pub enum SchedulerError {
    #[error("Failed to start scheduler: {0}")]
    StartFailed(String),
    #[error("Failed to stop scheduler: {0}")]
    StopFailed(String),
    #[error("Task execution failed: {0}")]
    TaskExecutionFailed(String),
    #[error("Invalid cron expression: {expression}")]
    InvalidCronExpression { expression: String },
}

/// Result type alias for bot operations
pub type BotResult<T> = Result<T, BotError>;

/// Result type aliases for specific error types
pub type DiscordResult<T> = Result<T, DiscordError>;
pub type SpotifyResult<T> = Result<T, SpotifyError>;
pub type ConfigResult<T> = Result<T, ConfigError>;
pub type PlaylistResult<T> = Result<T, PlaylistError>;
pub type MessageProcessingResult<T> = Result<T, MessageProcessingError>;
pub type DiscoveryResult<T> = Result<T, DiscoveryError>;
pub type SchedulerResult<T> = Result<T, SchedulerError>;