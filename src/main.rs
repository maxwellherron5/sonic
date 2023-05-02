use std::env;

mod spotify_client;


fn test_spotify() {
    let client_id = env::var("SPOTIFY_CLIENT_ID").expect("Expected a token in the environment");
    let client_secret = env::var("SPOTIFY_CLIENT_SECRET").expect("Expected a token in the environment");
    let foo = spotify_client::SpotifyClient::new(client_id, client_secret);
    foo.get_artist_details("0TnOYISbd1XYRBk9myaseg");
    println!("Think it worked!");
}


fn main() {
    test_spotify();
}
