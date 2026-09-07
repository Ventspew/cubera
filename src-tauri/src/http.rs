use std::sync::OnceLock;
use std::time::Duration;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Shared HTTP client tuned for Mojang / CDN downloads on macOS.
pub fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("MinecraftLauncher/2.2.2040 Cubera/0.1.0")
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(30))
            .tcp_nodelay(true)
            .build()
            .expect("failed to build HTTP client")
    })
}

pub async fn get_response(url: &str) -> Result<reqwest::Response, String> {
    let mut last_err = String::new();
    for attempt in 1..=6u32 {
        match client().get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return Ok(resp);
                }
                // Retry transient CDN / rate-limit errors
                if matches!(status.as_u16(), 408 | 425 | 429 | 500 | 502 | 503 | 504) {
                    last_err = format!("HTTP {status} for {url}");
                } else {
                    let body = resp.text().await.unwrap_or_default();
                    return Err(format!("Download mislukt ({status}) {url}: {body}"));
                }
            }
            Err(e) => {
                last_err = format!("Netwerkfout bij {url}: {e}");
            }
        }
        let delay_ms = 400u64 * (1u64 << (attempt - 1).min(4));
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }
    Err(format!("{last_err} (na 6 pogingen)"))
}
