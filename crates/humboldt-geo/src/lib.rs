pub mod bundled;
pub mod cables;
pub mod index;
pub mod models;
pub mod pip;

pub use bundled::bundled_spatial_index;
pub use cables::{bundled_submarine_cables, CableLandingPoint, SubmarineCable};
pub use index::SpatialIndex;
pub use models::{GeoPlace, GeoPolygon, ResolvedHierarchy};
pub use pip::{douglas_peucker_simplify, haversine_km, point_in_polygon, point_in_ring};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_resolve_kansas_city_hierarchy() {
        let index = bundled_spatial_index();

        // Coordinates in downtown Kansas City, MO: 39.0997 N, -94.5786 W
        let start = Instant::now();
        let resolved = index.resolve(39.0997, -94.5786);
        let elapsed = start.elapsed();

        // Must execute in < 5 ms offline per Section 7.1
        assert!(
            elapsed.as_millis() < 5,
            "Spatial resolution took {:?}, must be under 5ms",
            elapsed
        );

        assert_eq!(resolved.country.as_deref(), Some("United States"));
        assert_eq!(resolved.country_code.as_deref(), Some("US"));
        assert_eq!(resolved.state.as_deref(), Some("Missouri"));
        assert_eq!(resolved.state_code.as_deref(), Some("MO"));
        assert_eq!(resolved.county.as_deref(), Some("Jackson County"));
        assert_eq!(resolved.city.as_deref(), Some("Kansas City"));
        assert!(resolved.distance_to_city_km.unwrap() < 1.0);
    }

    #[test]
    fn test_resolve_london_hierarchy() {
        let index = bundled_spatial_index();

        // Coordinates in central London: 51.5074 N, -0.1278 W
        let resolved = index.resolve(51.5074, -0.1278);

        assert_eq!(resolved.country.as_deref(), Some("United Kingdom"));
        assert_eq!(resolved.country_code.as_deref(), Some("GB"));
        assert_eq!(resolved.state.as_deref(), Some("England"));
        assert_eq!(resolved.county.as_deref(), Some("Greater London"));
        assert_eq!(resolved.city.as_deref(), Some("London"));
    }

    #[test]
    fn test_open_ocean_resolution() {
        let index = bundled_spatial_index();

        // Coordinates in the South Pacific Ocean: -40.0 S, -140.0 W
        let resolved = index.resolve(-40.0, -140.0);

        assert!(resolved.country.is_none());
        assert!(resolved.state.is_none());
        assert!(resolved.county.is_none());
        assert!(resolved.city.is_none());
        assert_eq!(
            resolved.display_label(),
            "Open Ocean / International Territory"
        );
    }

    #[test]
    fn test_douglas_peucker_simplification() {
        // Linear path with small noise
        let pts = vec![
            [0.0, 0.0],
            [1.0, 0.0001],
            [2.0, -0.0001],
            [3.0, 0.0],
            [4.0, 1.0],
        ];

        let simplified = douglas_peucker_simplify(&pts, 0.001);
        // Collinear noisy points within epsilon=0.001 should be eliminated
        assert!(simplified.len() < pts.len());
        assert_eq!(simplified[0], [0.0, 0.0]);
        assert_eq!(*simplified.last().unwrap(), [4.0, 1.0]);
    }

    #[test]
    fn test_haversine_distance() {
        // Kansas City (39.0997, -94.5786) to Independence, MO (39.0911, -94.4155)
        let dist = haversine_km(39.0997, -94.5786, 39.0911, -94.4155);
        // Approx 14 km
        assert!(dist > 13.0 && dist < 16.0);
    }

    #[test]
    fn test_bundled_submarine_cables() {
        let cables = bundled_submarine_cables();
        assert!(cables.len() >= 10);
        let marea = cables.iter().find(|c| c.id == "marea").expect("MAREA cable missing");
        assert_eq!(marea.name, "MAREA");
        assert!(marea.landing_points.len() >= 2);
        assert!(marea.coordinates.len() >= 5);
    }
}
