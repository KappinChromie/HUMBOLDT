use serde::{Deserialize, Serialize};

/// Provenance tag designating proximity of the news report per Section 7.2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "entity")]
pub enum NewsProvenance {
    /// Level 3 query for specific city / town (e.g. "[Direct: Kansas City]")
    Direct(String),
    /// Level 2 fallback for county / district (e.g. "[Regional: Jackson County]")
    Regional(String),
    /// Level 1 fallback for state / province (e.g. "[Statewide: Missouri]")
    Statewide(String),
    /// Level 0 fallback for country (e.g. "[National: United States]")
    National(String),
}

impl NewsProvenance {
    pub fn badge_text(&self) -> String {
        match self {
            NewsProvenance::Direct(name) => format!("[Direct: {}]", name),
            NewsProvenance::Regional(name) => format!("[Regional: {}]", name),
            NewsProvenance::Statewide(name) => format!("[Statewide: {}]", name),
            NewsProvenance::National(name) => format!("[National: {}]", name),
        }
    }
}

/// Normalized news dispatch.
/// STRICT PRIVACY GUARDRAIL (Section 2): Full-text article bodies are categorically forbidden.
/// Content is strictly constrained to title, publication timestamp, source domain, and link URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsArticle {
    pub title: String,
    pub published_at: i64,
    pub source: String,
    pub url: String,
    pub provenance: NewsProvenance,
}
