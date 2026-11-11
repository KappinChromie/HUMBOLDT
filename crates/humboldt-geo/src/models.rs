use rstar::{RTreeObject, AABB};
use serde::{Deserialize, Serialize};

/// Administrative resolution result per Architecture Spec Section 7.1.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedHierarchy {
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub state: Option<String>,
    pub state_code: Option<String>,
    pub county: Option<String>,
    pub city: Option<String>,
    pub distance_to_city_km: Option<f64>,
}

impl ResolvedHierarchy {
    pub fn display_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(city) = &self.city {
            parts.push(city.as_str());
        }
        if let Some(county) = &self.county {
            parts.push(county.as_str());
        }
        if let Some(state) = &self.state {
            parts.push(state.as_str());
        }
        if let Some(country) = &self.country {
            parts.push(country.as_str());
        }

        if parts.is_empty() {
            "Open Ocean / International Territory".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Boundary polygon item indexed in R-Tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoPolygon {
    pub id: u32,
    /// Level: 0 = Country, 1 = State/Province, 2 = County/District
    pub level: u8,
    pub name: String,
    pub code: Option<String>,
    pub parent_code: Option<String>,
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
    /// Rings: rings[0] is outer boundary, rings[1..] are holes.
    /// Each ring is an array of [lon, lat] points.
    pub rings: Vec<Vec<[f64; 2]>>,
}

impl RTreeObject for GeoPolygon {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners([self.min_lon, self.min_lat], [self.max_lon, self.max_lat])
    }
}

/// Point-based Census place or populated place (>5,000 population).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoPlace {
    pub name: String,
    pub state_code: Option<String>,
    pub country_code: String,
    pub lat: f64,
    pub lon: f64,
    pub population: u32,
}

impl RTreeObject for GeoPlace {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.lon, self.lat])
    }
}
