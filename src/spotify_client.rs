use base64::{Engine as _, engine::general_purpose};
use rand::Rng;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

use crate::error::{SpotifyError, SpotifyResult};
use crate::models::{BotConfig, TrackInfo};

const API_URL: &str = "https://api.spotify.com/v1";
const TOKEN_REFRESH_BUFFER_SECONDS: u64 = 300; // Refresh token 5 minutes before expiry

#[derive(Clone)]
pub struct SpotifyClient {
    http_client: Client,
    access_token: Option<String>,
    refresh_token: Option<String>,
    config: BotConfig,
    token_expires_at: Option<SystemTime>,
}

impl SpotifyClient {
    pub fn new(config: &BotConfig) -> SpotifyClient {
        let http_client = Client::new();
        
        SpotifyClient {
            http_client,
            access_token: None,
            refresh_token: Some(config.spotify_refresh_token.clone()),
            config: config.clone(),
            token_expires_at: None,
        }
    }

    /// Initialize the client by obtaining an access token
    pub async fn initialize(&mut self) -> SpotifyResult<()> {
        self.refresh_access_token().await?;
        Ok(())
    }



    /// Refresh the access token using the refresh token
    pub async fn refresh_access_token(&mut self) -> SpotifyResult<()> {
        let refresh_token = self.refresh_token.as_ref()
            .ok_or_else(|| SpotifyError::AuthenticationFailed("No refresh token available".to_string()))?;

        let request_body = json!({
            "refresh_token": refresh_token,
            "grant_type": "refresh_token",
        });

        let formatted_credentials = format!("{}:{}", self.config.spotify_client_id, self.config.spotify_client_secret);
        let auth_header = format!("Basic {}", general_purpose::STANDARD.encode(&formatted_credentials));

        let response = self.http_client
            .post("https://accounts.spotify.com/api/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header(AUTHORIZATION, auth_header)
            .form(&request_body)
            .send()
            .await
            .map_err(|e| SpotifyError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            return Err(SpotifyError::TokenRefreshFailed(format!("HTTP {}: {}", status, error_text)));
        }

        let response_body: Value = response.json().await
            .map_err(|e| SpotifyError::JsonParsingError(e.to_string()))?;

        let access_token = response_body["access_token"]
            .as_str()
            .ok_or_else(|| SpotifyError::TokenRefreshFailed("No access token in response".to_string()))?
            .to_string();

        let expires_in = response_body["expires_in"]
            .as_u64()
            .unwrap_or(3600); // Default to 1 hour if not specified

        self.access_token = Some(access_token);
        self.token_expires_at = Some(SystemTime::now() + Duration::from_secs(expires_in));

        log::info!("Successfully refreshed Spotify access token, expires in {} seconds", expires_in);
        Ok(())
    }

    /// Check if the token needs to be refreshed (within buffer time of expiry)
    fn needs_token_refresh(&self) -> bool {
        match (self.access_token.as_ref(), self.token_expires_at) {
            (None, _) => true,
            (Some(_), None) => true,
            (Some(_), Some(expires_at)) => {
                let buffer_time = SystemTime::now() + Duration::from_secs(TOKEN_REFRESH_BUFFER_SECONDS);
                buffer_time >= expires_at
            }
        }
    }

    /// Ensure we have a valid access token, refreshing if necessary
    async fn ensure_valid_token(&mut self) -> SpotifyResult<()> {
        if self.needs_token_refresh() {
            log::debug!("Token needs refresh, refreshing now");
            self.refresh_access_token().await?;
        }
        Ok(())
    }

    /// Determine if an error should be retried and handle special cases
    async fn should_retry_error(&mut self, error: &SpotifyError) -> SpotifyResult<bool> {
        match error {
            SpotifyError::RateLimitExceeded { retry_after_ms } => {
                log::warn!("Rate limit exceeded, waiting {} ms before retry", retry_after_ms);
                sleep(Duration::from_millis(*retry_after_ms)).await;
                Ok(true)
            }
            SpotifyError::NetworkError(_) => Ok(true),
            SpotifyError::ApiRequestFailed { status, .. } => {
                // Retry on server errors (5xx) and some client errors
                Ok(*status >= 500 || *status == 429 || *status == 408)
            }
            SpotifyError::TokenExpired => {
                log::info!("Token expired during request, refreshing and retrying");
                self.refresh_access_token().await?;
                Ok(true)
            }
            _ => {
                log::debug!("Error not retryable: {:?}", error);
                Ok(false)
            }
        }
    }

    /// Calculate exponential backoff delay with jitter
    fn calculate_backoff_delay(&self, attempt: u32) -> u64 {
        let base_delay = self.config.retry_base_delay_ms;
        let max_delay = self.config.retry_max_delay_ms;
        
        // Exponential backoff: base_delay * 2^(attempt-1)
        let exponential_delay = base_delay * (2_u64.pow(attempt.saturating_sub(1)));
        let delay_with_cap = exponential_delay.min(max_delay);
        
        // Add jitter (±25% random variation)
        let jitter_range = delay_with_cap / 4; // 25% of the delay
        let jitter = rand::thread_rng().gen_range(0..=jitter_range * 2);
        let final_delay = delay_with_cap.saturating_sub(jitter_range) + jitter;
        
        final_delay.max(100) // Minimum 100ms delay
    }

    fn build_headers(&self) -> SpotifyResult<HeaderMap> {
        let access_token = self.access_token.as_ref()
            .ok_or_else(|| SpotifyError::AuthenticationFailed("No access token available".to_string()))?;

        let authorization = HeaderValue::from_str(&format!("Bearer {}", access_token))
            .map_err(|e| SpotifyError::AuthenticationFailed(format!("Invalid token format: {}", e)))?;
        
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        
        Ok(headers)
    }

    async fn make_get_request(&mut self, endpoint: &str) -> SpotifyResult<Value> {
        self.ensure_valid_token().await?;
        
        let mut attempt = 0;
        let max_attempts = self.config.max_retry_attempts;
        
        loop {
            attempt += 1;
            
            let headers = self.build_headers()?;
            let response = self.http_client
                .get(endpoint)
                .headers(headers)
                .send()
                .await
                .map_err(|e| SpotifyError::NetworkError(e.to_string()))?;

            match self.handle_response(response).await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if attempt >= max_attempts {
                        log::error!("Max retry attempts ({}) reached for GET request", max_attempts);
                        return Err(error);
                    }

                    let should_retry = self.should_retry_error(&error).await?;
                    if !should_retry {
                        return Err(error);
                    }

                    let delay_ms = self.calculate_backoff_delay(attempt);
                    log::debug!("Retrying GET request (attempt {}/{}) after {} ms delay", 
                              attempt, max_attempts, delay_ms);
                    sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    async fn make_post_request(&mut self, endpoint: &str, request_body: serde_json::Value) -> SpotifyResult<Value> {
        self.ensure_valid_token().await?;
        
        let mut attempt = 0;
        let max_attempts = self.config.max_retry_attempts;
        
        loop {
            attempt += 1;
            
            let headers = self.build_headers()?;
            let response = self.http_client
                .post(endpoint)
                .headers(headers)
                .json(&request_body)
                .send()
                .await
                .map_err(|e| SpotifyError::NetworkError(e.to_string()))?;

            match self.handle_response(response).await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if attempt >= max_attempts {
                        log::error!("Max retry attempts ({}) reached for POST request", max_attempts);
                        return Err(error);
                    }

                    let should_retry = self.should_retry_error(&error).await?;
                    if !should_retry {
                        return Err(error);
                    }

                    let delay_ms = self.calculate_backoff_delay(attempt);
                    log::debug!("Retrying POST request (attempt {}/{}) after {} ms delay", 
                              attempt, max_attempts, delay_ms);
                    sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    /// Handle HTTP response and convert to appropriate error types
    async fn handle_response(&self, response: reqwest::Response) -> SpotifyResult<Value> {
        let status = response.status();
        
        match status {
            StatusCode::OK | StatusCode::CREATED => {
                response.json().await
                    .map_err(|e| SpotifyError::JsonParsingError(e.to_string()))
            }
            StatusCode::UNAUTHORIZED => {
                Err(SpotifyError::TokenExpired)
            }
            StatusCode::TOO_MANY_REQUESTS => {
                let retry_after = response.headers()
                    .get("retry-after")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(1) * 1000; // Convert to milliseconds
                
                Err(SpotifyError::RateLimitExceeded { retry_after_ms: retry_after })
            }
            StatusCode::NOT_FOUND => {
                let error_text = response.text().await.unwrap_or_default();
                log::error!("Spotify API 404 response body: {}", error_text);
                if error_text.contains("track") {
                    Err(SpotifyError::TrackNotFound { track_id: "unknown".to_string() })
                } else if error_text.contains("playlist") {
                    Err(SpotifyError::PlaylistNotFound { playlist_id: "unknown".to_string() })
                } else {
                    Err(SpotifyError::ApiRequestFailed { 
                        status: status.as_u16(), 
                        message: if error_text.is_empty() { 
                            "404 Not Found - endpoint may not exist or resource not found".to_string() 
                        } else { 
                            error_text 
                        }
                    })
                }
            }
            StatusCode::FORBIDDEN => {
                let error_text = response.text().await.unwrap_or_default();
                if error_text.contains("playlist") {
                    Err(SpotifyError::PlaylistAccessDenied { playlist_id: "unknown".to_string() })
                } else {
                    Err(SpotifyError::ApiRequestFailed { 
                        status: status.as_u16(), 
                        message: error_text 
                    })
                }
            }
            _ => {
                let error_text = response.text().await.unwrap_or_default();
                Err(SpotifyError::ApiRequestFailed { 
                    status: status.as_u16(), 
                    message: error_text 
                })
            }
        }
    }

    /// Check if a track already exists in a playlist
    pub async fn check_track_exists_in_playlist(&mut self, playlist_id: &str, track_uri: &str) -> SpotifyResult<bool> {
        let mut offset = 0;
        let limit = 100; // Maximum allowed by Spotify API
        
        loop {
            let endpoint = format!("{}/playlists/{}/tracks?offset={}&limit={}&fields=items(track(uri))", 
                                 API_URL, playlist_id, offset, limit);
            
            let response = self.make_get_request(&endpoint).await?;
            
            let items = response["items"].as_array()
                .ok_or_else(|| SpotifyError::JsonParsingError("Invalid playlist tracks response".to_string()))?;
            
            // Check if the track URI exists in this batch
            for item in items {
                if let Some(track) = item["track"].as_object() {
                    if let Some(uri) = track["uri"].as_str() {
                        if uri == track_uri {
                            return Ok(true);
                        }
                    }
                }
            }
            
            // If we got fewer items than the limit, we've reached the end
            if items.len() < limit {
                break;
            }
            
            offset += limit;
        }
        
        Ok(false)
    }

    /// Get all tracks from a playlist
    pub async fn get_playlist_tracks(&mut self, playlist_id: &str) -> SpotifyResult<Vec<TrackInfo>> {
        let mut tracks = Vec::new();
        let mut offset = 0;
        let limit = 100;
        
        loop {
            let endpoint = format!("{}/playlists/{}/tracks?offset={}&limit={}&fields=items(track(id,uri,name,artists(name),album(name),duration_ms,external_urls,popularity,preview_url,explicit))", 
                                 API_URL, playlist_id, offset, limit);
            
            let response = self.make_get_request(&endpoint).await?;
            
            let items = response["items"].as_array()
                .ok_or_else(|| SpotifyError::JsonParsingError("Invalid playlist tracks response".to_string()))?;
            
            for item in items {
                if let Some(track_data) = item["track"].as_object() {
                    if let Ok(track_info) = self.parse_track_info(track_data) {
                        tracks.push(track_info);
                    }
                }
            }
            
            if items.len() < limit {
                break;
            }
            
            offset += limit;
        }
        
        Ok(tracks)
    }

    /// Parse track information from Spotify API response
    fn parse_track_info(&self, track_data: &serde_json::Map<String, Value>) -> SpotifyResult<TrackInfo> {
        let id = track_data["id"].as_str()
            .ok_or_else(|| SpotifyError::JsonParsingError("Missing track ID".to_string()))?
            .to_string();
        
        let uri = track_data["uri"].as_str()
            .ok_or_else(|| SpotifyError::JsonParsingError("Missing track URI".to_string()))?
            .to_string();
        
        let name = track_data["name"].as_str()
            .ok_or_else(|| SpotifyError::JsonParsingError("Missing track name".to_string()))?
            .to_string();
        
        let artists = track_data["artists"].as_array()
            .ok_or_else(|| SpotifyError::JsonParsingError("Missing artists array".to_string()))?
            .iter()
            .filter_map(|artist| artist["name"].as_str())
            .map(|name| name.to_string())
            .collect();
        
        let album = track_data["album"]["name"].as_str()
            .ok_or_else(|| SpotifyError::JsonParsingError("Missing album name".to_string()))?
            .to_string();
        
        let duration_ms = track_data["duration_ms"].as_u64()
            .ok_or_else(|| SpotifyError::JsonParsingError("Missing duration".to_string()))? as u32;
        
        let external_urls = track_data["external_urls"].as_object()
            .map(|urls| {
                urls.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        
        let popularity = track_data["popularity"].as_u64().map(|p| p as u8);
        let preview_url = track_data["preview_url"].as_str().map(|s| s.to_string());
        let explicit = track_data["explicit"].as_bool().unwrap_or(false);
        
        Ok(TrackInfo {
            id,
            uri,
            name,
            artists,
            album,
            duration_ms,
            external_urls,
            popularity,
            preview_url,
            explicit,
        })
    }

    /// Get track information by track ID
    pub async fn get_track_info(&mut self, track_id: &str) -> SpotifyResult<TrackInfo> {
        let endpoint = format!("{}/tracks/{}", API_URL, track_id);
        let response = self.make_get_request(&endpoint).await?;
        
        let track_data = response.as_object()
            .ok_or_else(|| SpotifyError::JsonParsingError("Invalid track response".to_string()))?;
        
        self.parse_track_info(track_data)
    }

    /// Search for tracks using a query string
    /// Returns up to `limit` tracks matching the search query
    pub async fn search_tracks(&mut self, query: &str, limit: u32) -> SpotifyResult<Vec<TrackInfo>> {
        // URL encode the query
        let encoded_query = query.replace(" ", "%20");
        let limit = limit.min(50); // Spotify's max limit is 50
        
        let endpoint = format!(
            "{}/search?q={}&type=track&limit={}",
            API_URL,
            encoded_query,
            limit
        );
        
        log::debug!("Searching tracks: {}", endpoint);
        
        let response = self.make_get_request(&endpoint).await?;
        
        // Parse search results
        let tracks_obj = response["tracks"]["items"].as_array()
            .ok_or_else(|| SpotifyError::JsonParsingError("Invalid search response".to_string()))?;
        
        let mut tracks = Vec::new();
        for track_data in tracks_obj {
            if let Some(track_obj) = track_data.as_object() {
                if let Ok(track_info) = self.parse_track_info(track_obj) {
                    tracks.push(track_info);
                }
            }
        }
        
        log::debug!("Found {} tracks for query: {}", tracks.len(), query);
        Ok(tracks)
    }

    /// Add a track to a playlist with duplicate checking
    pub async fn add_track_to_playlist(&mut self, playlist_id: &str, track_uri: &str) -> SpotifyResult<()> {
        // Check if track already exists in playlist
        if self.check_track_exists_in_playlist(playlist_id, track_uri).await? {
            return Err(SpotifyError::InvalidTrackUri { 
                uri: format!("Track {} already exists in playlist", track_uri) 
            });
        }

        let endpoint = format!("{}/playlists/{}/tracks", API_URL, playlist_id);
        let request_body = json!({ "uris": [track_uri] });
        
        self.make_post_request(&endpoint, request_body).await?;
        log::info!("Successfully added track {} to playlist {}", track_uri, playlist_id);
        Ok(())
    }

    /// Add a track to a playlist without duplicate checking (for internal use)
    pub async fn add_track_to_playlist_force(&mut self, playlist_id: &str, track_uri: &str) -> SpotifyResult<()> {
        let endpoint = format!("{}/playlists/{}/tracks", API_URL, playlist_id);
        let request_body = json!({ "uris": [track_uri] });
        
        self.make_post_request(&endpoint, request_body).await?;
        log::info!("Successfully added track {} to playlist {} (forced)", track_uri, playlist_id);
        Ok(())
    }

    /// Get recommendations based on seed tracks
    pub async fn get_recommendations(&mut self, seed_tracks: Vec<String>) -> SpotifyResult<Vec<TrackInfo>> {
        if seed_tracks.is_empty() {
            return Err(SpotifyError::ApiRequestFailed { 
                status: 400, 
                message: "At least one seed track is required".to_string() 
            });
        }

        // Spotify API allows up to 5 seed tracks
        let limited_seeds: Vec<String> = seed_tracks.into_iter().take(5).collect();
        let seed_tracks_param = limited_seeds.join(",");
        
        // Build the recommendations endpoint with required parameters
        // Note: Spotify recommendations API is very particular about parameters
        let endpoint = format!(
            "{}/recommendations?limit=20&seed_tracks={}", 
            API_URL, 
            seed_tracks_param
        );
        
        log::info!("Requesting recommendations from: {}", endpoint);
        log::info!("Seed tracks: {:?}", limited_seeds);
        
        // Ensure we have a valid token before making the request
        self.ensure_valid_token().await?;
        
        log::debug!("Making GET request to recommendations endpoint");
        let response = self.make_get_request(&endpoint).await?;
        log::debug!("Received response from recommendations endpoint");
        
        let tracks_array = response["tracks"].as_array()
            .ok_or_else(|| SpotifyError::JsonParsingError("Invalid recommendations response".to_string()))?;
        
        let mut recommendations = Vec::new();
        for track_data in tracks_array {
            if let Some(track_obj) = track_data.as_object() {
                if let Ok(track_info) = self.parse_track_info(track_obj) {
                    recommendations.push(track_info);
                }
            }
        }
        
        log::info!("Retrieved {} recommendations using {} seed tracks", 
                  recommendations.len(), limited_seeds.len());
        Ok(recommendations)
    }

    /// Get recommendations with additional parameters for fine-tuning
    pub async fn get_recommendations_with_params(
        &mut self, 
        seed_tracks: Vec<String>,
        target_energy: Option<f32>,
        target_danceability: Option<f32>,
        target_valence: Option<f32>,
        limit: Option<u32>
    ) -> SpotifyResult<Vec<TrackInfo>> {
        if seed_tracks.is_empty() {
            return Err(SpotifyError::ApiRequestFailed { 
                status: 400, 
                message: "At least one seed track is required".to_string() 
            });
        }

        let limited_seeds: Vec<String> = seed_tracks.into_iter().take(5).collect();
        let seed_tracks_param = limited_seeds.join(",");
        let limit = limit.unwrap_or(20).min(100); // Max 100 tracks
        
        let mut endpoint = format!(
            "{}/recommendations?seed_tracks={}&limit={}&market=US", 
            API_URL, 
            seed_tracks_param,
            limit
        );
        
        // Add optional audio feature parameters
        if let Some(energy) = target_energy {
            endpoint.push_str(&format!("&target_energy={:.2}", energy.clamp(0.0, 1.0)));
        }
        if let Some(danceability) = target_danceability {
            endpoint.push_str(&format!("&target_danceability={:.2}", danceability.clamp(0.0, 1.0)));
        }
        if let Some(valence) = target_valence {
            endpoint.push_str(&format!("&target_valence={:.2}", valence.clamp(0.0, 1.0)));
        }
        
        let response = self.make_get_request(&endpoint).await?;
        
        let tracks_array = response["tracks"].as_array()
            .ok_or_else(|| SpotifyError::JsonParsingError("Invalid recommendations response".to_string()))?;
        
        let mut recommendations = Vec::new();
        for track_data in tracks_array {
            if let Some(track_obj) = track_data.as_object() {
                if let Ok(track_info) = self.parse_track_info(track_obj) {
                    recommendations.push(track_info);
                }
            }
        }
        
        log::info!("Retrieved {} recommendations with custom parameters using {} seed tracks", 
                  recommendations.len(), limited_seeds.len());
        Ok(recommendations)
    }

    /// Replace all tracks in a playlist with new tracks
    pub async fn replace_playlist_tracks(&mut self, playlist_id: &str, track_uris: Vec<String>) -> SpotifyResult<()> {
        if track_uris.is_empty() {
            return Err(SpotifyError::ApiRequestFailed { 
                status: 400, 
                message: "At least one track URI is required".to_string() 
            });
        }

        let endpoint = format!("{}/playlists/{}/tracks", API_URL, playlist_id);
        let request_body = json!({ "uris": track_uris });
        
        // Use PUT request to replace all tracks
        self.ensure_valid_token().await?;
        
        let mut attempt = 0;
        let max_attempts = self.config.max_retry_attempts;
        
        loop {
            attempt += 1;
            
            let headers = self.build_headers()?;
            let response = self.http_client
                .put(&endpoint)
                .headers(headers)
                .json(&request_body)
                .send()
                .await
                .map_err(|e| SpotifyError::NetworkError(e.to_string()))?;

            match self.handle_response(response).await {
                Ok(_) => break,
                Err(error) => {
                    if attempt >= max_attempts {
                        log::error!("Max retry attempts ({}) reached for PUT request", max_attempts);
                        return Err(error);
                    }

                    let should_retry = self.should_retry_error(&error).await?;
                    if !should_retry {
                        return Err(error);
                    }

                    let delay_ms = self.calculate_backoff_delay(attempt);
                    log::debug!("Retrying PUT request (attempt {}/{}) after {} ms delay", 
                              attempt, max_attempts, delay_ms);
                    sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
        log::info!("Successfully replaced playlist {} with {} tracks", playlist_id, track_uris.len());
        Ok(())
    }
}
