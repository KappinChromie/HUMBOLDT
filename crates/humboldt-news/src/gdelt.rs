use crate::models::{NewsArticle, NewsProvenance};
use chrono::NaiveDateTime;
use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, Result};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub const GDELT_DOC_API_URL: &str = "https://api.gdeltproject.org/api/v2/doc/doc";

#[derive(Debug, Deserialize)]
struct GdeltResponse {
    pub articles: Option<Vec<GdeltArticleItem>>,
}

#[derive(Debug, Deserialize)]
struct GdeltArticleItem {
    pub title: Option<String>,
    pub url: Option<String>,
    pub seendate: Option<String>,
    pub domain: Option<String>,
}

/// Parses GDELT DOC API JSON response into normalized NewsArticle records.
pub fn parse_gdelt_json(json_str: &str, provenance: NewsProvenance) -> Result<Vec<NewsArticle>> {
    let resp: GdeltResponse = serde_json::from_str(json_str)
        .map_err(HumboldtError::Serialization)?;

    let raw_articles = match resp.articles {
        Some(list) => list,
        None => return Ok(Vec::new()),
    };

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut articles = Vec::with_capacity(raw_articles.len());

    for item in raw_articles {
        let title = match item.title {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => continue,
        };

        let url = match item.url {
            Some(u) if !u.trim().is_empty() => u.trim().to_string(),
            _ => continue,
        };

        let source = item
            .domain
            .unwrap_or_else(|| "gdeltproject.org".to_string());

        // Parse GDELT seendate format "20261109T183000Z"
        let published_at = item
            .seendate
            .and_then(|sd| {
                NaiveDateTime::parse_from_str(&sd, "%Y%m%dT%H%M%SZ")
                    .ok()
                    .map(|dt| dt.and_utc().timestamp_millis())
            })
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

/// Fetches articles from GDELT DOC API matching a spatial or entity query.
pub async fn fetch_gdelt_articles(
    client: &reqwest::Client,
    query: &str,
    provenance: NewsProvenance,
) -> Result<Vec<NewsArticle>> {
    let url = format!(
        "{}?query={}&mode=artlist&format=json&maxrecords=20&sourcelang=eng",
        GDELT_DOC_API_URL,
        urlencoding::encode(query)
    );

    let response = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("GDELT request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "GDELT HTTP error: status {}",
            response.status()
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to read GDELT body: {}", e)))?;

    let articles = parse_gdelt_json(&body, provenance)?;
    info!("Ingested {} articles from GDELT for query '{}'.", articles.len(), query);
    Ok(articles)
}
