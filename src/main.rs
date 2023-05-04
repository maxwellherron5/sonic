use std::env;
mod spotify_client;
mod discord_client;


// // fn main() {
// //     let client_id = env::var("SPOTIFY_CLIENT_ID").expect("Expected a token in the environment");
// //     let client_secret = env::var("SPOTIFY_CLIENT_SECRET").expect("Expected a token in the environment");
// //     let spotify_client = spotify_client::SpotifyClient::new(client_id, client_secret);
// //     let x = spotify_client.get_track_uri("11dFghVXANMlKmJXsNCbNl").replace("\"", "");
// //     spotify_client.add_to_playlist(&x);
// // }


#[tokio::main]
async fn main() {
    discord_client::start_bot().await;
}
