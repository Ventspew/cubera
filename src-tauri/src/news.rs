use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItem {
    pub title: String,
    pub tag: String,
    pub date: String,
    pub text: String,
    pub image_url: Option<String>,
    pub read_more_url: Option<String>,
}

#[derive(Deserialize)]
struct MojangNewsRoot {
    entries: Option<Vec<MojangEntry>>,
}

#[derive(Deserialize)]
struct MojangEntry {
    title: Option<String>,
    tag: Option<String>,
    date: Option<String>,
    text: Option<String>,
    #[serde(rename = "playPageImage")]
    play_page_image: Option<MojangImage>,
    #[serde(rename = "newsPageImage")]
    news_page_image: Option<MojangImage>,
    #[serde(rename = "readMoreLink")]
    read_more_link: Option<String>,
    #[serde(default)]
    news: Option<bool>,
}

#[derive(Deserialize)]
struct MojangImage {
    url: Option<String>,
}

pub async fn fetch_minecraft_news() -> Result<Vec<NewsItem>, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://launchercontent.mojang.com/news.json")
        .header("User-Agent", "Cubera/0.1.0 (Minecraft Launcher)")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("News feed unavailable ({})", resp.status()));
    }

    let root: MojangNewsRoot = resp.json().await.map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for entry in root.entries.unwrap_or_default() {
        if entry.news == Some(false) {
            continue;
        }
        let title = entry.title.unwrap_or_else(|| "Minecraft".into());
        let text = entry.text.unwrap_or_default();
        if title.is_empty() && text.is_empty() {
            continue;
        }
        let image = entry
            .news_page_image
            .or(entry.play_page_image)
            .and_then(|i| i.url)
            .map(|u| {
                if u.starts_with("http") {
                    u
                } else {
                    format!("https://launchercontent.mojang.com{u}")
                }
            });
        items.push(NewsItem {
            title,
            tag: entry.tag.unwrap_or_else(|| "News".into()),
            date: entry.date.unwrap_or_default(),
            text,
            image_url: image,
            read_more_url: entry.read_more_link,
        });
        if items.len() >= 12 {
            break;
        }
    }
    Ok(items)
}
