use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{Value, json};
use open;

const API_URL: &str = "https://api.spotify.com/v1";
// TODO this will eventually be user configurable
const PLAYLIST_ID: &str = "3nf65T5wXvLYLvT6xvXoLf";

pub struct SpotifyClient {
    http_client: Client,
    access_token: String,
}

impl SpotifyClient {
    pub fn new(client_id: String, client_secret: String) -> SpotifyClient {
        let http_client = Client::new();
        let access_token = SpotifyClient::get_access_token(&client_id, &client_secret, &http_client).unwrap();
        SpotifyClient {http_client, access_token}
    }

    fn get_access_token_(client_id: &String, client_secret: &String, http_client: &Client) -> Result<String, Box<dyn std::error::Error>> {

        // AUTHORIZATION SCOPES
        // let params = [
        //     ("client_id", client_id.to_string()),
        //     ("response_type", "code".to_string()),
        //     ("scope", "playlist-modify-public".to_string()),
        //     ("redirect_uri", "http://localhost:7777/callback".to_string())
        // ];   
        // let params = [
        //     ("client_id", ""),
        //     ("response_type", "code"),
        //     ("scope", "playlist-modify-public"),
        //     ("redirect_uri", "http://localhost:7777/callback")
        // ];       
        // let resp = http_client
        //   .get("https://accounts.spotify.com/authorize?")
        //   .query(&params)  
        //   .send()?;
        // println!("{:?}", resp);
        // open::that("https://accounts.spotify.com/authorize?scope=playlist-modify-public&response_type=code&redirect_uri=http://localhost:7777/callback&client_id=Da3a31c215fdf4067a602ca39c55e73b4")?;
        // AUTHORIZATION SCOPES

        let request_body = json!(
            {
                "grant_type": "client_credentials",
                "scope": "playlist-modify-public",
                "client_id": client_id,
                "client_secret": client_secret,
            }
        );
    
        let response = http_client
          .post("https://accounts.spotify.com/api/token")
          .header("Content-Type", "application/x-www-form-urlencoded")
          .form(&request_body)
          .send()?;
        
        let response_body: Value = response.json()?;
        println!("{:?}", response_body);
        return Ok(response_body["access_token"].to_string());
    }

    fn build_headers(&self) -> HeaderMap {
        let authorization: HeaderValue = HeaderValue::from_str(&format!("Bearer {}", &self.access_token.replace("\"", ""))).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        return headers
    }

    fn make_get_request(&self, endpoint: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let headers: HeaderMap = self.build_headers();
        let response = self.http_client
          .get(endpoint)
          .headers(headers)
          .send()?;

        let response_body: Value = response.json()?;
        // println!("{:?}", response_body);
        // Ok(())
        Ok(response_body)
    }

    fn make_post_request(&self, endpoint: &str, request_body: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        let headers: HeaderMap = self.build_headers();
        let response = self.http_client
          .post(endpoint)
          .headers(headers)
          .json(&request_body)
          .send()?;
        
        let response_body: Value = response.json()?;
        println!("{:?}", response_body);
        Ok(())
    }

    pub fn get_artist_details(&self, artist_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let endpoint = format!("{API_URL}/artists/{artist_id}");
        let response = self.make_get_request(&endpoint);
        Ok(())
    }

    pub fn get_track_uri(&self, track_id: &str) -> String {
        let endpoint = format!("{API_URL}/tracks/{track_id}");
        let response = self.make_get_request(&endpoint).unwrap();
        let uri = response["uri"].to_string();
        return uri
    }

    pub fn add_to_playlist(&self, track_uri: &str) {
        let endpoint = format!("{API_URL}/playlists/{PLAYLIST_ID}/tracks");

        let request_body = json!(
            {
                "uris": [
                    track_uri
                ]
            }
        );
        println!("Request body: {:?}, {}", request_body, endpoint);
        let response = self.make_post_request(&endpoint, request_body);
    }
}
