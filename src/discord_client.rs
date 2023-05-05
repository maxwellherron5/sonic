use std::env;

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use url::Url;

use crate::spotify_client;

struct Handler {
    spotify_client: spotify_client::SpotifyClient,
}

impl Default for Handler {
    fn default() -> Handler {
        Handler {
            spotify_client: spotify_client::SpotifyClient::new(),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if !msg.author.bot {
            // Try to see if a URL is in the message
            let url = Url::parse(&msg.content);
            match url {
                Ok(url) => {
                    let id = url.path().split("/").nth(2);
                    // let client_id = env::var("SPOTIFY_CLIENT_ID").expect("Expected a token in the environment");
                    // let client_secret = env::var("SPOTIFY_CLIENT_SECRET").expect("Expected a token in the environment");
                    // let spotify_client = spotify_client::SpotifyClient::new(client_id, client_secret);
                    self.spotify_client.get_track_uri(id.unwrap());
                }
                Err(_) => println!("Message does not contain a URL"),
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

// pub struct DiscordClient {
//     spotify_client: SpotifyifyClient,
//     token: String,
//     intents: GatewayIntents,
// }

// impl DiscordClient {
//     pub fn new(spotify_client: SpotifyifyClient, token: String, intents: GatewayIntents) -> DiscordClient {
//         DiscordClient {spotify_client, token, intents}
//     }

//     pub fn start(&self) {
//         // Create a new instance of the Client, logging in as a bot. This will
//         // automatically prepend your bot token with "Bot ", which is a requirement
//         // by Discord for bot users.
//         let mut client =
//         Client::builder(self.token, self.intents).event_handler(Handler).await.expect("Err creating client");

//         if let Err(why) = client.start().await {
//             println!("Client error: {:?}", why);
//         }
//     }
// }

pub async fn start_bot() {
    // Configure the client with your Discord bot token in the environment.
    let token =
        env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Create a new instance of the Client, logging in as a bot. This will
    // automatically prepend your bot token with "Bot ", which is a requirement
    // by Discord for bot users.
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            spotify_client: spotify_client::SpotifyClient::new(),
        })
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
