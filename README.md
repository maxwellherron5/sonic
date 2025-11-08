# Sonic - Your Discord Spotify Bot!

A Discord bot that monitors channels for Spotify track links and automatically adds them to a collaborative playlist. Generates weekly discovery playlists based on collected tracks.

## Features

- Monitors Discord channels for Spotify track URLs
- Automatically adds tracks to a collaborative Spotify playlist
- Prevents duplicate track additions
- Generates weekly discovery playlists using search-based recommendations
- Scheduled playlist generation via cron expressions

## Requirements

- Rust (latest stable)
- Discord bot token
- Spotify application credentials (Client ID, Client Secret, Refresh Token)
- Two Spotify playlists (collaborative and discovery)

## Configuration

Set the following environment variables:

```
DISCORD_TOKEN=<discord_bot_token>
TARGET_CHANNEL_ID=<discord_channel_id>
SPOTIFY_CLIENT_ID=<spotify_client_id>
SPOTIFY_CLIENT_SECRET=<spotify_client_secret>
SPOTIFY_REFRESH_TOKEN=<spotify_refresh_token>
COLLABORATIVE_PLAYLIST_ID=<playlist_id>
DISCOVERY_PLAYLIST_ID=<playlist_id>
WEEKLY_SCHEDULE_CRON=0 0 12 * * MON
```

## Build

```bash
cargo build --release
```

## Run

```bash
cargo run --release
```

## Utilities

- `cargo run --bin config_test` - Validate configuration
- `cargo run --bin integration_test` - Test API connectivity
- `cargo run --bin test_playlists` - Test playlist access
- `cargo run --bin generate_discovery` - Manually trigger discovery generation
- `cargo run --bin get_spotify_token` - Generate Spotify refresh token

## Architecture

- `discord_client` - Discord API integration
- `spotify_client` - Spotify Web API client
- `message_processor` - URL extraction and validation
- `playlist_manager` - Playlist operations
- `discovery_generator` - Weekly playlist generation
- `scheduler` - Cron-based task scheduling

## License

MIT
