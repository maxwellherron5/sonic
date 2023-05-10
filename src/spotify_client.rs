use base64;
use std::env;

use log::{error, info};
use open;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::StatusCode;
use serde_json::{json, Value};
use url::Url;

const API_URL: &str = "https://api.spotify.com/v1";
// TODO this will eventually be user configurable
const PLAYLIST_ID: &str = "3nf65T5wXvLYLvT6xvXoLf";

#[derive(Clone)]
pub struct SpotifyClient {
    http_client: Client,
    access_token: String,
    client_id: String,
    client_secret: String,
    authorization_code: String,
}

impl SpotifyClient {
    pub fn new() -> SpotifyClient {
        let client_id = env::var("SPOTIFY_CLIENT_ID")
            .expect("Expected a spotify client ID the environment");
        let client_secret = env::var("SPOTIFY_CLIENT_SECRET")
            .expect("Expected a spotify client secret in the environment");
        let authorization_code = env::var("SPOTIFY_AUTH_CODE")
            .expect("Expected a spotify authorization code");
        let http_client = Client::new();
        // SpotifyClient::authorize_app(&client_id, &http_client);
        let access_token = SpotifyClient::get_access_token(
            &client_id,
            &client_secret,
            &http_client,
            &authorization_code,
        )
        .unwrap();
        // let access_token = String::new();
        SpotifyClient {
            http_client,
            access_token,
            client_id,
            client_secret,
            authorization_code,
        }
    }

    fn authorize_app(
        client_id: &String,
        http_client: &Client,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let params = [
            ("client_id", client_id.to_string()),
            ("response_type", "code".to_string()),
            ("scope", "playlist-modify-public".to_string()),
            ("redirect_uri", "http://127.0.0.1:5000/callback".to_string()),
        ];
        let response = http_client
            .get("https://accounts.spotify.com/authorize?")
            .query(&params)
            .send()?;

        let url_returned: &str = response.url().query().unwrap();
        let url_parsed = url::form_urlencoded::parse(url_returned.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect::<Vec<(String, String)>>();

        let parsed_path = url_parsed[0].1.to_owned();
        info!("{:?}", parsed_path);
        open::that(parsed_path)?;

        return Ok(());
    }

    fn get_access_token(
        client_id: &String,
        client_secret: &String,
        http_client: &Client,
        authorization_code: &String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let request_body = json!(
            {
                "code": authorization_code,
                "grant_type": "authorization_code",
                "redirect_uri": "http://127.0.0.1:5000/callback",
            }
        );
        let formatted_credentials = format!("{}:{}", client_id, client_secret);
        let auth_header =
            format!("Basic {}", base64::encode(&formatted_credentials));
        let response = http_client
            .post("https://accounts.spotify.com/api/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header(AUTHORIZATION, auth_header)
            .form(&request_body)
            .send()?;

        let response_body: Value = response.json()?;
        return Ok(response_body["access_token"].to_string());
    }

    fn build_headers(&self) -> HeaderMap {
        let authorization: HeaderValue = HeaderValue::from_str(&format!(
            "Bearer {}",
            &self.access_token.replace("\"", "")
        ))
        .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        headers
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        return headers;
    }

    fn make_get_request(
        &mut self,
        endpoint: &str,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let headers: HeaderMap = self.build_headers();
        let response =
            self.http_client.get(endpoint).headers(headers).send()?;

        match response.status() {
            StatusCode::OK => {
                let response_body: Value = response.json()?;
                return Ok(response_body);
            }
            StatusCode::UNAUTHORIZED => {
                self.access_token = SpotifyClient::get_access_token(
                    &self.client_id,
                    &self.client_secret,
                    &self.http_client,
                    &self.authorization_code,
                )
                .unwrap();
                let response_body: Value = response.json()?;
                return Ok(response_body);
            }
            _ => {
                let response_body: Value = response.json()?;
                return Ok(response_body);
            }
        }
        // let response_body: Value = response.json()?;
    }

    fn make_post_request(
        &self,
        endpoint: &str,
        request_body: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let headers: HeaderMap = self.build_headers();
        let response = self
            .http_client
            .post(endpoint)
            .headers(headers)
            .json(&request_body)
            .send()?;

        let response_body: Value = response.json()?;
        Ok(())
    }

    pub fn get_artist_details(
        &mut self,
        artist_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let endpoint = format!("{API_URL}/artists/{artist_id}");
        let response = self.make_get_request(&endpoint);
        Ok(())
    }

    pub fn get_track_uri(&mut self, track_id: &str) -> String {
        let endpoint = format!("{API_URL}/tracks/{track_id}");
        let response = self.make_get_request(&endpoint).unwrap();
        let uri = response["uri"].to_string().replace("\"", "");
        return uri;
    }

    pub fn add_to_playlist(&self, track_uri: &str) {
        let endpoint = format!("{API_URL}/playlists/{PLAYLIST_ID}/tracks");
        let request_body = json!({ "uris": [track_uri] });
        let response = self.make_post_request(&endpoint, request_body);
    }
}

fn get_access_token(
    client_id: &String,
    client_secret: &String,
    http_client: &Client,
    authorization_code: &String,
) -> Result<String, Box<dyn std::error::Error>> {
    let request_body = json!(
        {
            "code": authorization_code,
            "grant_type": "authorization_code",
            "redirect_uri": "http://127.0.0.1:5000/callback",
        }
    );
    let formatted_credentials = format!("{}:{}", client_id, client_secret);
    let auth_header =
        format!("Basic {}", base64::encode(&formatted_credentials));
    let response = http_client
        .post("https://accounts.spotify.com/api/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header(AUTHORIZATION, auth_header)
        .form(&request_body)
        .send()?;

    let response_body: Value = response.json()?;
    return Ok(response_body["access_token"].to_string());
}
