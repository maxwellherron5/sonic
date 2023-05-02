use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{Value, json};


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

    fn get_access_token(client_id: &String, client_secret: &String, http_client: &Client) -> Result<String, Box<dyn std::error::Error>> {
        let request_body = json!(
            {
                "grant_type": "client_credentials",
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
        println!("{:?}", response_body);
        // Ok(())
        Ok(response_body)
    }

    // fn make_post_request(&self, endpoint: &str) -> Result<(), Box<dyn std::error::Error>> {

    // }

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

    pub fn add_to_playlist(&self, track_id: &str) {
        let endpoint = format!("{API_URL}/playlists/{track_id}");
    }
}
