pub mod engine;
pub mod gdelt;
pub mod models;
pub mod rss;

pub use engine::NewsHierarchyEngine;
pub use gdelt::{fetch_gdelt_articles, parse_gdelt_json, GDELT_DOC_API_URL};
pub use models::{NewsArticle, NewsProvenance};
pub use rss::{fetch_google_news_rss, parse_google_news_rss, GOOGLE_NEWS_RSS_URL};

#[cfg(test)]
mod tests {
    use super::*;

    fn get_fixture(filename: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures")
            .join(filename);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("Failed to read fixture: {:?}", path))
    }

    #[test]
    fn test_parse_gdelt_from_fixture() {
        let json = get_fixture("gdelt_city_news.json");
        let prov = NewsProvenance::Direct("Kansas City".to_string());
        let articles = parse_gdelt_json(&json, prov).expect("Failed to parse GDELT fixture");

        assert_eq!(articles.len(), 3);
        assert_eq!(
            articles[0].title,
            "Kansas City Streetcar Extension Testing Begins on Main Street"
        );
        assert_eq!(articles[0].source, "kansascity.com");
        assert_eq!(
            articles[0].provenance,
            NewsProvenance::Direct("Kansas City".to_string())
        );
        assert_eq!(articles[0].provenance.badge_text(), "[Direct: Kansas City]");
    }

    #[test]
    fn test_parse_google_news_rss_from_fixture() {
        let xml = get_fixture("google_news_rss.xml");
        let prov = NewsProvenance::Direct("Kansas City".to_string());
        let articles = parse_google_news_rss(&xml, prov).expect("Failed to parse Google News RSS fixture");

        assert_eq!(articles.len(), 2);
        assert_eq!(
            articles[0].title,
            "Downtown Kansas City transit infrastructure development accelerates"
        );
        assert_eq!(articles[0].source, "KSHB 41");
        assert_eq!(
            articles[1].title,
            "Jackson County infrastructure improvement bond initiative approved"
        );
        assert_eq!(articles[1].source, "Kansas City Star");
    }

    #[test]
    fn test_provenance_badge_formatting() {
        assert_eq!(
            NewsProvenance::Direct("Kansas City".to_string()).badge_text(),
            "[Direct: Kansas City]"
        );
        assert_eq!(
            NewsProvenance::Regional("Jackson County".to_string()).badge_text(),
            "[Regional: Jackson County]"
        );
        assert_eq!(
            NewsProvenance::Statewide("Missouri".to_string()).badge_text(),
            "[Statewide: Missouri]"
        );
        assert_eq!(
            NewsProvenance::National("United States".to_string()).badge_text(),
            "[National: United States]"
        );
    }
}
