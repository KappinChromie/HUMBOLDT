use crate::gdelt::fetch_gdelt_articles;
use crate::models::{NewsArticle, NewsProvenance};
use crate::rss::fetch_google_news_rss;
use humboldt_core::Result;
use humboldt_geo::ResolvedHierarchy;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::info;

struct CachedNews {
    fetched_at: Instant,
    articles: Vec<NewsArticle>,
}

/// Hierarchical News Aggregation Engine matching Architecture Spec Section 7.2.
#[derive(Clone)]
pub struct NewsHierarchyEngine {
    client: reqwest::Client,
    cache: Arc<RwLock<HashMap<String, CachedNews>>>,
    cache_ttl: Duration,
}

impl NewsHierarchyEngine {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(7200), // 2-hour cache per Section 6.2
        }
    }

    /// Fetches news articles using the Section 7.2 hierarchical fallback algorithm.
    pub async fn fetch_news_for_hierarchy(
        &self,
        hierarchy: &ResolvedHierarchy,
    ) -> Result<Vec<NewsArticle>> {
        let cache_key = hierarchy.display_label();

        // Check 2-hour cache
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(&cache_key) {
                if entry.fetched_at.elapsed() < self.cache_ttl {
                    info!("Returning {} cached articles for '{}'.", entry.articles.len(), cache_key);
                    return Ok(entry.articles.clone());
                }
            }
        }

        let mut collected: Vec<NewsArticle> = Vec::new();
        let mut seen_urls: HashSet<String> = HashSet::new();

        let add_articles = |articles: Vec<NewsArticle>,
                            collected: &mut Vec<NewsArticle>,
                            seen: &mut HashSet<String>| {
            for art in articles {
                if !seen.contains(&art.url) {
                    seen.insert(art.url.clone());
                    collected.push(art);
                }
            }
        };

        // Step 1: Level 3 (City / Town Query)
        if let Some(city) = &hierarchy.city {
            let direct_prov = NewsProvenance::Direct(city.clone());
            let query = format!("\"{}\" sourcelang:eng", city);

            match fetch_gdelt_articles(&self.client, &query, direct_prov.clone()).await {
                Ok(articles) => {
                    add_articles(articles, &mut collected, &mut seen_urls);
                }
                Err(e) => {
                    info!("City-level GDELT query failed, trying Google News RSS: {}", e);
                }
            }

            // If GDELT produces 0 results for municipality, invoke Google News RSS
            if collected.is_empty() {
                let state_str = hierarchy.state.as_deref().unwrap_or("");
                let rss_query = if state_str.is_empty() {
                    city.clone()
                } else {
                    format!("{} {}", city, state_str)
                };

                if let Ok(rss_arts) =
                    fetch_google_news_rss(&self.client, &rss_query, direct_prov).await
                {
                    add_articles(rss_arts, &mut collected, &mut seen_urls);
                }
            }
        }

        // Step 2: Fallback to Level 2 (County / District) if fewer than 3 verified articles
        if collected.len() < 3 {
            if let Some(county) = &hierarchy.county {
                let regional_prov = NewsProvenance::Regional(county.clone());
                let query = format!("\"{}\" sourcelang:eng", county);

                if let Ok(county_arts) =
                    fetch_gdelt_articles(&self.client, &query, regional_prov).await
                {
                    add_articles(county_arts, &mut collected, &mut seen_urls);
                }
            }
        }

        // Step 3: Fallback to Level 1 (State / Province) if still fewer than 3 articles
        if collected.len() < 3 {
            if let Some(state) = &hierarchy.state {
                let state_prov = NewsProvenance::Statewide(state.clone());
                let query = format!("\"{}\" sourcelang:eng", state);

                if let Ok(state_arts) =
                    fetch_gdelt_articles(&self.client, &query, state_prov).await
                {
                    add_articles(state_arts, &mut collected, &mut seen_urls);
                }
            }
        }

        // Step 4: Fallback to Level 0 (Country) if still empty
        if collected.is_empty() {
            if let Some(country) = &hierarchy.country {
                let nat_prov = NewsProvenance::National(country.clone());
                let query = format!("\"{}\" sourcelang:eng", country);

                if let Ok(country_arts) =
                    fetch_gdelt_articles(&self.client, &query, nat_prov).await
                {
                    add_articles(country_arts, &mut collected, &mut seen_urls);
                }
            }
        }

        // Sort descending by published timestamp
        collected.sort_by_key(|a| std::cmp::Reverse(a.published_at));

        // Store in 2-hour cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                cache_key,
                CachedNews {
                    fetched_at: Instant::now(),
                    articles: collected.clone(),
                },
            );
        }

        Ok(collected)
    }
}
