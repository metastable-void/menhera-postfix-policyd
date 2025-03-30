
use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use regex::Regex;
use serde::Deserialize;

#[derive(Deserialize)]
struct MyResult {
    username: String,
    members: Vec<String>,
}

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "".to_string());
    let port = port.parse::<u16>().unwrap_or(13088);

    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("Failed to bind to address");

    println!("Listening on port {}", port);

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                println!("Accepted connection from {}", addr);
                // Handle the connection in a separate task
                tokio::spawn(async move {
                    let wildcard_pattern = Regex::new(r"^[-a-zA-Z0-9._]+@([a-z0-9]+)\.(?:h09\.eu|mnhr\.org)$").unwrap();
                    let domains = vec![
                        "menhera.org",
                        "h09.eu",
                    ];

                    let mut reader = BufReader::new(socket);
                    let mut line = String::new();

                    while let Ok(bytes_read) = reader.read_line(&mut line).await {
                        if bytes_read == 0 {
                            // Connection closed
                            break;
                        }

                        let parts = line.trim().split(' ').collect::<Vec<_>>();
                        if parts.len() == 2 {
                            let key = parts[0];
                            let value = parts[1];
                            if key == "get" {
                                let mut results = Err(());
                                let parts = value.split('@').collect::<Vec<_>>();
                                if parts.len() != 2 {
                                    println!("Invalid recipient format: {}", value);
                                    continue;
                                }

                                let username = parts[0].to_string();
                                let domain = parts[1].to_string();
                                drop(parts);

                                if domains.iter().any(|&d| d == &domain) {
                                    results = check_user(&username).await;
                                } else if wildcard_pattern.is_match(value) {
                                    let username = wildcard_pattern.captures(value).unwrap();
                                    let username = username.get(0).unwrap().as_str();
                                    results = check_user(username).await;
                                }

                                if results.is_ok() {
                                    let results = results.unwrap();
                                    let results_joined = results.join(" ");
                                    let _ = reader.write_all(format!("200 {}\n", results_joined).as_bytes()).await;
                                } else {
                                    let _ = reader.write_all(b"500 error\n").await;
                                }
                            }
                        } else {
                            break;
                        }
                        line.clear();
                    }
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn check_user(username: &str) -> Result<Vec<String>, ()> {
    let mut url = reqwest::Url::parse("https://accounts.menhera.org/api/v1/username/get").unwrap();
    url.query_pairs_mut().append_pair("username", username);
    let client = reqwest::Client::new();
    let response = client.get(url).send().await;
    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let json: Result<MyResult, _> = resp.json().await;
                match json {
                    Ok(data) => {
                        if data.members.is_empty() {
                            return Ok(vec![data.username]);
                        }
                        return Ok(data.members);
                    }
                    Err(e) => {
                        eprintln!("Error parsing JSON: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error checking username: {}", e);
        }
    }
    Err(())
}
