use crate::models::{GeoPlace, GeoPolygon, ResolvedHierarchy};
use crate::pip::{haversine_km, point_in_polygon};
use rstar::{RTree, AABB};

/// Local R-Tree Administrative Spatial Index matching Architecture Spec Section 7.1.
pub struct SpatialIndex {
    countries: RTree<GeoPolygon>,
    states: RTree<GeoPolygon>,
    counties: RTree<GeoPolygon>,
    cities: RTree<GeoPolygon>,
    places: RTree<GeoPlace>,
}

impl SpatialIndex {
    pub fn new(
        countries: Vec<GeoPolygon>,
        states: Vec<GeoPolygon>,
        counties: Vec<GeoPolygon>,
        cities: Vec<GeoPolygon>,
        places: Vec<GeoPlace>,
    ) -> Self {
        Self {
            countries: RTree::bulk_load(countries),
            states: RTree::bulk_load(states),
            counties: RTree::bulk_load(counties),
            cities: RTree::bulk_load(cities),
            places: RTree::bulk_load(places),
        }
    }

    pub fn all_countries(&self) -> Vec<GeoPolygon> {
        self.countries.iter().cloned().collect()
    }

    pub fn all_states(&self) -> Vec<GeoPolygon> {
        self.states.iter().cloned().collect()
    }

    pub fn all_counties(&self) -> Vec<GeoPolygon> {
        self.counties.iter().cloned().collect()
    }

    pub fn all_cities(&self) -> Vec<GeoPolygon> {
        self.cities.iter().cloned().collect()
    }

    pub fn all_places(&self) -> Vec<GeoPlace> {
        self.places.iter().cloned().collect()
    }

    /// Resolves WGS84 coordinates into administrative hierarchy (`Country → State → County → City`).
    /// Architecture constraint: Must execute in < 5 ms offline.
    pub fn resolve(&self, lat: f64, lon: f64) -> ResolvedHierarchy {
        let mut hierarchy = ResolvedHierarchy::default();
        let query_point = [lon, lat];

        let point_env = AABB::from_point(query_point);

        // Level 0: Country
        for candidate in self.countries.locate_in_envelope_intersecting(&point_env) {
            if point_in_polygon(lon, lat, candidate) {
                hierarchy.country = Some(candidate.name.clone());
                hierarchy.country_code = candidate.code.clone();
                break;
            }
        }

        // Level 1: State / Province
        for candidate in self.states.locate_in_envelope_intersecting(&point_env) {
            if point_in_polygon(lon, lat, candidate) {
                hierarchy.state = Some(candidate.name.clone());
                hierarchy.state_code = candidate.code.clone();
                if hierarchy.country.is_none() && candidate.parent_code.is_some() {
                    hierarchy.country_code = candidate.parent_code.clone();
                }
                break;
            }
        }

        // Level 2: County / District
        for candidate in self.counties.locate_in_envelope_intersecting(&point_env) {
            if point_in_polygon(lon, lat, candidate) {
                hierarchy.county = Some(candidate.name.clone());
                break;
            }
        }

        // Level 3: Municipal Cities (Polygons)
        for candidate in self.cities.locate_in_envelope_intersecting(&point_env) {
            if point_in_polygon(lon, lat, candidate) {
                hierarchy.city = Some(candidate.name.clone());
                hierarchy.distance_to_city_km = Some(0.0);
                if hierarchy.state_code.is_none() {
                    hierarchy.state_code = candidate.parent_code.clone();
                }
                break;
            }
        }

        // Level 3 Fallback: Census Places / Cities (>5,000 population within 25 km radius)
        if hierarchy.city.is_none() {
            const SEARCH_RADIUS_KM: f64 = 25.0;
            const DEG_DELTA: f64 = 0.35;
            let search_env = AABB::from_corners(
                [lon - DEG_DELTA, lat - DEG_DELTA],
                [lon + DEG_DELTA, lat + DEG_DELTA],
            );

            let mut closest_place: Option<(&GeoPlace, f64)> = None;

            for place in self.places.locate_in_envelope_intersecting(&search_env) {
                if place.population >= 5000 {
                    let dist = haversine_km(lat, lon, place.lat, place.lon);
                    if dist <= SEARCH_RADIUS_KM {
                        match closest_place {
                            Some((_, min_dist)) if dist < min_dist => {
                                closest_place = Some((place, dist));
                            }
                            None => {
                                closest_place = Some((place, dist));
                            }
                            _ => {}
                        }
                    }
                }
            }

            if let Some((place, dist)) = closest_place {
                hierarchy.city = Some(place.name.clone());
                hierarchy.distance_to_city_km = Some(dist);
                if hierarchy.state_code.is_none() {
                    hierarchy.state_code = place.state_code.clone();
                }
                if hierarchy.country_code.is_none() {
                    hierarchy.country_code = Some(place.country_code.clone());
                }
            }
        }

        hierarchy
    }
}
