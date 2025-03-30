
use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use regex::Regex;

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

                    let mut recipients_results = vec![];

                    while let Ok(bytes_read) = reader.read_line(&mut line).await {
                        if bytes_read == 0 {
                            // Connection closed
                            break;
                        }

                        let parts = line.trim().split('=').collect::<Vec<_>>();
                        if parts.len() == 2 {
                            let key = parts[0];
                            let value = parts[1];
                            println!("Received: {} = {}", key, value);
                            if key == "recipient" {
                                let mut permit = false;
                                let parts = value.split('@').collect::<Vec<_>>();
                                if parts.len() != 2 {
                                    println!("Invalid recipient format: {}", value);
                                    recipients_results.push(false);
                                    continue;
                                }

                                let username = parts[0].to_string();
                                let domain = parts[1].to_string();
                                drop(parts);

                                if domains.iter().any(|&d| d == &domain) {
                                    permit = check_user(&username).await;
                                } else if wildcard_pattern.is_match(value) {
                                    let username = wildcard_pattern.captures(value).unwrap();
                                    let username = username.get(0).unwrap().as_str();
                                    permit = check_user(username).await;
                                }
                                println!("Valid recipient domain: {}", &domain);
                                recipients_results.push(permit);
                            }
                        } else {
                            // end of request
                            // Check if all recipients are valid
                            let all_valid = recipients_results.len() > 0 && recipients_results.iter().all(|&valid| valid);
                            if all_valid {
                                println!("All recipients are valid");
                                // Send response
                                let response = format!("action=permit Ok\n\n");
                                if let Err(e) = reader.get_mut().write_all(response.as_bytes()).await {
                                    eprintln!("Failed to send response: {}", e);
                                }
                            } else {
                                println!("Not all recipients are valid");
                                // Send error response
                                let response = format!("action=defer_if_permit Service temporarily unavailable\n\n");
                                if let Err(e) = reader.get_mut().write_all(response.as_bytes()).await {
                                    eprintln!("Failed to send error response: {}", e);
                                }
                            }
                            // Reset recipients results for the next request
                            recipients_results.clear();
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

async fn check_user(username: &str) -> bool {
    let mut url = reqwest::Url::parse("https://accounts.menhera.org/api/v1/username/get").unwrap();
    url.query_pairs_mut().append_pair("username", username);
    let client = reqwest::Client::new();
    let response = client.get(url).send().await;
    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                return true;
            }
        }
        Err(e) => {
            eprintln!("Error checking username: {}", e);
        }
    }
    false
}
