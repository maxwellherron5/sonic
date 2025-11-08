use std::io::{self, Write};

#[tokio::main]
async fn main() {
    println!("==============================================");
    println!("Spotify Refresh Token Generator");
    println!("==============================================\n");

    // Get client ID and secret
    print!("Enter your Spotify Client ID: ");
    io::stdout().flush().unwrap();
    let mut client_id = String::new();
    io::stdin().read_line(&mut client_id).unwrap();
    let client_id = client_id.trim();

    print!("Enter your Spotify Client Secret: ");
    io::stdout().flush().unwrap();
    let mut client_secret = String::new();
    io::stdin().read_line(&mut client_secret).unwrap();
    let client_secret = client_secret.trim();

    // Generate authorization URL
    let redirect_uri = "http://localhost:8888/callback";
    let scopes = "playlist-modify-public playlist-modify-private playlist-read-private playlist-read-collaborative";
    let encoded_scopes = scopes.replace(" ", "%20");
    
    let auth_url = format!(
        "https://accounts.spotify.com/authorize?client_id={}&response_type=code&redirect_uri={}&scope={}",
        client_id, redirect_uri, encoded_scopes
    );

    println!("\n==============================================");
    println!("Step 1: Authorize the Application");
    println!("==============================================");
    println!("\nOpen this URL in your browser:\n");
    println!("{}\n", auth_url);
    println!("After authorizing, you'll be redirected to a URL that looks like:");
    println!("http://localhost:8888/callback?code=AUTHORIZATION_CODE\n");

    // Try to open the URL automatically
    if let Err(_) = open::that(&auth_url) {
        println!("(Could not open browser automatically - please copy the URL above)");
    } else {
        println!("(Browser should open automatically)");
    }

    println!("\n==============================================");
    println!("Step 2: Copy the Authorization Code");
    println!("==============================================");
    print!("\nPaste the FULL redirect URL here: ");
    io::stdout().flush().unwrap();
    let mut redirect_url = String::new();
    io::stdin().read_line(&mut redirect_url).unwrap();
    let redirect_url = redirect_url.trim();

    // Extract authorization code from URL
    let auth_code = if let Some(code_start) = redirect_url.find("code=") {
        let code = &redirect_url[code_start + 5..];
        // Remove any trailing parameters
        if let Some(amp_pos) = code.find('&') {
            &code[..amp_pos]
        } else {
            code
        }
    } else {
        eprintln!("\n❌ Error: Could not find authorization code in URL");
        eprintln!("Make sure you pasted the full redirect URL");
        std::process::exit(1);
    };

    println!("\n==============================================");
    println!("Step 3: Exchange Code for Tokens");
    println!("==============================================");
    println!("\nExchanging authorization code for tokens...");

    // Exchange authorization code for tokens
    let client = reqwest::Client::new();
    let params = [
        ("grant_type", "authorization_code"),
        ("code", auth_code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];

    match client
        .post("https://accounts.spotify.com/api/token")
        .form(&params)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(json) => {
                        if let Some(refresh_token) = json.get("refresh_token").and_then(|v| v.as_str()) {
                            println!("\n✅ Success! Here is your refresh token:\n");
                            println!("==============================================");
                            println!("{}", refresh_token);
                            println!("==============================================\n");
                            
                            println!("Add this to your .env file as:");
                            println!("SPOTIFY_REFRESH_TOKEN={}\n", refresh_token);
                            
                            // Optionally save to .env file
                            print!("Would you like to add this to your .env file now? (y/n): ");
                            io::stdout().flush().unwrap();
                            let mut answer = String::new();
                            io::stdin().read_line(&mut answer).unwrap();
                            
                            if answer.trim().to_lowercase() == "y" {
                                match update_env_file(refresh_token) {
                                    Ok(_) => println!("✅ Updated .env file successfully!"),
                                    Err(e) => eprintln!("⚠️  Could not update .env file: {}", e),
                                }
                            }
                            
                            println!("\n🎉 Setup complete! You can now run the bot.");
                        } else {
                            eprintln!("\n❌ Error: No refresh token in response");
                            eprintln!("Response: {}", json);
                        }
                    }
                    Err(e) => {
                        eprintln!("\n❌ Error parsing response: {}", e);
                    }
                }
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                eprintln!("\n❌ Error: Spotify API returned status {}", status);
                eprintln!("Response: {}", error_text);
                eprintln!("\nCommon issues:");
                eprintln!("- Make sure your Client ID and Client Secret are correct");
                eprintln!("- Verify the redirect URI in your Spotify app settings is: {}", redirect_uri);
                eprintln!("- Check that you copied the full authorization code");
            }
        }
        Err(e) => {
            eprintln!("\n❌ Error making request: {}", e);
        }
    }
}

fn update_env_file(refresh_token: &str) -> io::Result<()> {
    use std::fs;
    
    let env_path = ".env";
    
    // Read existing .env file or create new one
    let content = if std::path::Path::new(env_path).exists() {
        fs::read_to_string(env_path)?
    } else {
        // Copy from .env.example if it exists
        if std::path::Path::new(".env.example").exists() {
            fs::read_to_string(".env.example")?
        } else {
            String::new()
        }
    };
    
    // Check if SPOTIFY_REFRESH_TOKEN already exists
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut found = false;
    
    for line in &mut lines {
        if line.starts_with("SPOTIFY_REFRESH_TOKEN=") {
            *line = format!("SPOTIFY_REFRESH_TOKEN={}", refresh_token);
            found = true;
            break;
        }
    }
    
    if !found {
        lines.push(format!("SPOTIFY_REFRESH_TOKEN={}", refresh_token));
    }
    
    // Write back to file
    fs::write(env_path, lines.join("\n") + "\n")?;
    
    Ok(())
}
