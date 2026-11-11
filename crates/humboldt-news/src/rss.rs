use crate::models::{NewsArticle, NewsProvenance};
use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, Result};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub const GOOGLE_NEWS_RSS_URL: &str = "https://news.google.com/rss/search";

/// Parses Google News RSS XML string using `feed-rs`.
pub fn parse_google_news_rss(xml_str: &str, provenance: NewsProvenance) -> Result<Vec<NewsArticle>> {
    let feed = feed_rs::parser::parse(xml_str.as_bytes())
        .map_err(|e| HumboldtError::Serialization(serde::de::Error::custom(format!("RSS parse error: {}", e))))?;

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut articles = Vec::with_capacity(feed.entries.len());

    for entry in feed.entries {
        let raw_title = match entry.title {
            Some(t) if !t.content.trim().is_empty() => t.content.trim().to_string(),
            _ => continue,
        };

        let url = entry
            .links
            .first()
            .map(|l| l.href.clone())
            .unwrap_or_default();

        if url.is_empty() {
            continue;
        }

        // Google News titles typically format as "Headline Title - Source Name"
        let (title, source) = if let Some(idx) = raw_title.rfind(" - ") {
            let (t, s) = raw_title.split_at(idx);
            (t.trim().to_string(), s.trim_start_matches(" - ").trim().to_string())
        } else {
            (raw_title, "Google News".to_string())
        };

        let published_at = entry
            .published
            .or(entry.updated)
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(now_ms);

        articles.push(NewsArticle {
            title,
            published_at,
            source,
            url,
            provenance: provenance.clone(),
        });
    }

    Ok(articles)
}

/// Fetches Google News RSS articles for a query.
pub async fn fetch_google_news_rss(
    client: &reqwest::Client,
    query: &str,
    provenance: NewsProvenance,
) -> Result<Vec<NewsArticle>> {
    let url = format!(
        "{}?q={}&hl=en-US&gl=US&ceid=US:en",
        GOOGLE_NEWS_RSS_URL,
        urlencoding::encode(query)
    );

    let response = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("Google News RSS request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "Google News RSS error: status {}",
            response.status()
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to read RSS body: {}", e)))?;

    let articles = parse_google_news_rss(&body, provenance)?;
    info!(
        "Ingested {} articles from Google News RSS for query '{}'.",
        articles.len(),
        query
    );
    Ok(articles)
}
