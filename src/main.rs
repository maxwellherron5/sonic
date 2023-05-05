mod discord_client;
mod spotify_client;

#[tokio::main]
async fn main() {
    discord_client::start_bot().await;
}
