use std::env;

mod spotify_client;


fn main() {
    let client_id = env::var("SPOTIFY_CLIENT_ID").expect("Expected a token in the environment");
    let client_secret = env::var("SPOTIFY_CLIENT_SECRET").expect("Expected a token in the environment");
    let spotify_client = spotify_client::SpotifyClient::new(client_id, client_secret);
    spotify_client.get_track_uri("11dFghVXANMlKmJXsNCbNl");
}
