use crate::index::SpatialIndex;
use crate::models::{GeoPlace, GeoPolygon};
use serde::Deserialize;

#[derive(Deserialize)]
struct BundledData {
    countries: Vec<GeoPolygon>,
    states: Vec<GeoPolygon>,
    counties: Vec<GeoPolygon>,
    cities: Vec<GeoPolygon>,
    places: Vec<GeoPlace>,
}

const BUNDLED_JSON: &str = include_str!("bundled_boundaries.json");

/// Builds the preprocessed bundled spatial index matching Section 7.1.
/// Backed by authentic Natural Earth 1:110m Admin 0/1 and US Census TIGER geometries.
pub fn bundled_spatial_index() -> SpatialIndex {
    let data: BundledData = serde_json::from_str(BUNDLED_JSON)
        .expect("bundled_boundaries.json must be valid GeoPolygon and GeoPlace JSON");
    SpatialIndex::new(data.countries, data.states, data.counties, data.cities, data.places)
}
