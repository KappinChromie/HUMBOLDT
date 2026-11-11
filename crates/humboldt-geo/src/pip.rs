use crate::models::GeoPolygon;

/// Ray-casting Point-in-Ring test using the Jordan curve theorem.
/// ring vertices format: [lon, lat]
pub fn point_in_ring(lon: f64, lat: f64, ring: &[[f64; 2]]) -> bool {
    let n = ring.len();
    if n < 3 {
        return false;
    }

    let mut inside = false;
    let mut j = n - 1;

    for i in 0..n {
        let xi = ring[i][0];
        let yi = ring[i][1];
        let xj = ring[j][0];
        let yj = ring[j][1];

        let intersect = ((yi > lat) != (yj > lat))
            && (lon < (xj - xi) * (lat - yi) / (yj - yi) + xi);

        if intersect {
            inside = !inside;
        }
        j = i;
    }

    inside
}

/// Point-in-Polygon test accounting for boundary rings across Polygons and MultiPolygons.
pub fn point_in_polygon(lon: f64, lat: f64, poly: &GeoPolygon) -> bool {
    // 1. Fast bounding box rejection
    if lon < poly.min_lon || lon > poly.max_lon || lat < poly.min_lat || lat > poly.max_lat {
        return false;
    }

    if poly.rings.is_empty() {
        return false;
    }

    // 2. Point-in-ring test across all polygonal boundary rings
    for ring in &poly.rings {
        if point_in_ring(lon, lat, ring) {
            return true;
        }
    }

    false
}

/// Computes Great-Circle distance between two WGS84 coordinates in kilometers.
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0088;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();

    let a = (d_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (d_lon / 2.0).sin().powi(2);

    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    EARTH_RADIUS_KM * c
}

/// Simplifies a 2D polygonal ring using the Douglas-Peucker algorithm.
/// Section 7.1 requires polygon simplification (epsilon = 0.001) for compact packaging.
pub fn douglas_peucker_simplify(points: &[[f64; 2]], epsilon: f64) -> Vec<[f64; 2]> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let mut dmax = 0.0;
    let mut index = 0;
    let end = points.len() - 1;

    for (i, pt) in points.iter().enumerate().take(end).skip(1) {
        let d = perpendicular_distance(pt, &points[0], &points[end]);
        if d > dmax {
            index = i;
            dmax = d;
        }
    }

    if dmax > epsilon {
        let rec_results1 = douglas_peucker_simplify(&points[0..=index], epsilon);
        let rec_results2 = douglas_peucker_simplify(&points[index..=end], epsilon);

        let mut result = rec_results1;
        result.pop(); // Remove duplicate point at boundary
        result.extend(rec_results2);
        result
    } else {
        vec![points[0], points[end]]
    }
}

fn perpendicular_distance(point: &[f64; 2], line_start: &[f64; 2], line_end: &[f64; 2]) -> f64 {
    let dx = line_end[0] - line_start[0];
    let dy = line_end[1] - line_start[1];

    let mag = (dx * dx + dy * dy).sqrt();
    if mag == 0.0 {
        let px = point[0] - line_start[0];
        let py = point[1] - line_start[1];
        return (px * px + py * py).sqrt();
    }

    let u = ((point[0] - line_start[0]) * dx + (point[1] - line_start[1]) * dy) / (mag * mag);
    let clamped_u = u.clamp(0.0, 1.0);

    let ix = line_start[0] + clamped_u * dx;
    let iy = line_start[1] + clamped_u * dy;

    let dist_x = point[0] - ix;
    let dist_y = point[1] - iy;
    (dist_x * dist_x + dist_y * dist_y).sqrt()
}
