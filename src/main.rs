// use std::env;

// use url::Url;
// use serenity::async_trait;
// use serenity::model::channel::Message;
// use serenity::model::gateway::Ready;
// use serenity::prelude::*;

mod spotify_client;
mod discord_client;


// struct Handler;


// #[async_trait]
// impl EventHandler for Handler {
//     async fn message(&self, ctx: Context, msg: Message) {
//         if !msg.author.bot{
//             // Try to see if a URL is in the message
//             let url = Url::parse(&msg.content);
//             match url {
//                 Ok(url) => {
//                     let id = url.path().split("/").nth(2);
//                     // spotify_client::SpotifyClient::new().get_track_uri(id.unwrap());
//                 },
//                 Err(_) => println!("Message does not contain a URL")
//             }
//         }
//     }

//     async fn ready(&self, _: Context, ready: Ready) {
//         println!("{} is connected!", ready.user.name);
//     }
// }


#[tokio::main]
async fn main() {
    discord_client::start_bot().await;
}
