// use std::env;
// mod spotify_client;
// mod discord_client;
// use url::Url;

// // fn main() {
// //     let client_id = env::var("SPOTIFY_CLIENT_ID").expect("Expected a token in the environment");
// //     let client_secret = env::var("SPOTIFY_CLIENT_SECRET").expect("Expected a token in the environment");
// //     let spotify_client = spotify_client::SpotifyClient::new(client_id, client_secret);
// //     let x = spotify_client.get_track_uri("11dFghVXANMlKmJXsNCbNl").replace("\"", "");
// //     spotify_client.add_to_playlist(&x);
// // }


// #[tokio::main]
// async fn main() {
//     // let client_id = env::var("SPOTIFY_CLIENT_ID").expect("Expected a token in the environment");
//     // let client_secret = env::var("SPOTIFY_CLIENT_SECRET").expect("Expected a token in the environment");
//     // let spotify_client = spotify_client::SpotifyClient::new(client_id, client_secret);

//     discord_client::start_bot().await;
// }

use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{Value, json};



use std::env;

use url::Url;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

mod spotify_client;
const API_URL: &str = "https://api.spotify.com/v1";
struct Handler {
    http_client: Client,
    access_token: String,
}

impl Handler {
    fn new() -> Handler {
        let client_id = env::var("SPOTIFY_CLIENT_ID").expect("Expected a token in the environment");
        let client_secret = env::var("SPOTIFY_CLIENT_SECRET").expect("Expected a token in the environment");
        let http_client = Client::new();
        let access_token = Handler::get_access_token(&client_id, &client_secret, &http_client).unwrap();
        return Handler {http_client, access_token}
    }

    pub fn get_track_uri(&self, track_id: &str) -> String {
        let endpoint = format!("{API_URL}/tracks/{track_id}");
        let response = self.make_get_request(&endpoint).unwrap();
        let uri = response["uri"].to_string();
        println!("{:?} URI HERE", uri);
        return uri
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

    fn build_headers(&self) -> HeaderMap {
        let authorization: HeaderValue = HeaderValue::from_str(&format!("Bearer {}", &self.access_token.replace("\"", ""))).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        return headers
    }

    fn get_access_token(client_id: &String, client_secret: &String, http_client: &Client) -> Result<String, Box<dyn std::error::Error>> {
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
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if !msg.author.bot{
            // Try to see if a URL is in the message
            let url = Url::parse(&msg.content);
            match url {
                Ok(url) => {
                    let id = url.path().split("/").nth(2);
                    self.get_track_uri(id.unwrap());
                },
                Err(_) => println!("Message does not contain a URL")
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}


// #[tokio::main]
// async fn main() {
//     // Configure the client with your Discord bot token in the environment.
//     let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
//     // Set gateway intents, which decides what events the bot will be notified about
//     let intents = GatewayIntents::GUILD_MESSAGES
//         | GatewayIntents::DIRECT_MESSAGES
//         | GatewayIntents::MESSAGE_CONTENT;

//     // Create a new instance of the Client, logging in as a bot. This will
//     // automatically prepend your bot token with "Bot ", which is a requirement
//     // by Discord for bot users.
//     
//     if let Err(why) = client.start().await {
//         println!("Client error: {:?}", why);
//     }
// }

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let handler = Handler::new();
    let mut client =
      serenity::Client::builder(&token, intents).event_handler(handler).await.expect("Err creating client");
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }  
}
