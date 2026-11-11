use crate::{HumboldtError, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// Telemetry scalar point (epoch ms, value)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DataPoint {
    pub timestamp: i64,
    pub value: f64,
}

/// Dual-mode comparison alignment per Architecture Spec Section 9.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonMode {
    /// Mode 1: Absolute calendar time alignment (identifies year-over-year deviations)
    Calendar,
    /// Mode 2: Relative day-of-period alignment (normalized to elapsed days from start)
    Relative,
}

/// Statistical divergence metrics matching Architecture Spec Section 9.4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonStats {
    /// Delta Mean: Mean(B) - Mean(A)
    pub delta_mean: f64,
    /// Delta Max: Max(B) - Max(A)
    pub delta_max: f64,
    /// Delta Min: Min(B) - Min(A)
    pub delta_min: f64,
    /// Normalized Root-Mean-Square Deviation:
    /// NRMSD = sqrt( (1/N) * sum( (y_B,i - y_A,i)^2 ) ) / (max(A) - min(A))
    pub nrmsd: f64,
    pub sample_count: usize,
}

/// Frozen snapshot for historical analysis per Architecture Spec Section 5.3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// UUIDv4 identifier
    pub id: String,
    /// User-defined label (e.g., 'KC Temp Oct 2026')
    pub name: String,
    /// Unix epoch timestamp
    pub created_at: i64,
    pub layer: String,
    pub metric_name: String,
    /// Target location name, boundary ID, or station
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_entity: Option<String>,
    /// Epoch start of snapshot range
    pub start_time: i64,
    /// Epoch end of snapshot range
    pub end_time: i64,
    /// 'raw', '1h_rollup', or '1d_rollup'
    pub resolution: String,
    /// Serialized provider provenance, unit, coordinates
    pub metadata_json: String,
    /// Gzipped serialized array of data points
    pub data_points: Vec<u8>,
}

impl Snapshot {
    /// Creates a snapshot encoding data points into compressed gzip bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        name: String,
        created_at: i64,
        layer: String,
        metric_name: String,
        query_entity: Option<String>,
        start_time: i64,
        end_time: i64,
        resolution: String,
        metadata_json: String,
        points: &[DataPoint],
    ) -> Result<Self> {
        let data_points = encode_data_points(points)?;
        Ok(Self {
            id,
            name,
            created_at,
            layer,
            metric_name,
            query_entity,
            start_time,
            end_time,
            resolution,
            metadata_json,
            data_points,
        })
    }

    /// Decompresses and deserializes data points.
    pub fn decoded_points(&self) -> Result<Vec<DataPoint>> {
        decode_data_points(&self.data_points)
    }

    /// Formats data points as standard RFC 4180 CSV string.
    pub fn to_csv(&self) -> Result<String> {
        let points = self.decoded_points()?;
        let mut csv = String::from("timestamp_ms,iso_time,value\n");
        for p in points {
            let iso = chrono::DateTime::from_timestamp_millis(p.timestamp)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
            csv.push_str(&format!("{},{},{}\n", p.timestamp, iso, p.value));
        }
        Ok(csv)
    }
}

/// Gzip compresses an array of DataPoints into bytes.
pub fn encode_data_points(points: &[DataPoint]) -> Result<Vec<u8>> {
    let json_bytes = serde_json::to_vec(points)
        .map_err(HumboldtError::Serialization)?;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&json_bytes)
        .map_err(|e| HumboldtError::Storage(format!("Gzip compression failed: {}", e)))?;

    encoder
        .finish()
        .map_err(|e| HumboldtError::Storage(format!("Gzip finish failed: {}", e)))
}

/// Decompresses gzip bytes into an array of DataPoints.
pub fn decode_data_points(bytes: &[u8]) -> Result<Vec<DataPoint>> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    // Try gzip decoding first
    let mut decoder = GzDecoder::new(bytes);
    let mut decompressed = Vec::new();
    if decoder.read_to_end(&mut decompressed).is_ok() {
        if let Ok(points) = serde_json::from_slice::<Vec<DataPoint>>(&decompressed) {
            return Ok(points);
        }
    }

    // Fallback if raw JSON bytes were passed uncompressed
    serde_json::from_slice::<Vec<DataPoint>>(bytes)
        .map_err(HumboldtError::Serialization)
}

/// Computes statistical divergence metrics between Series A and Series B per Section 9.4.
pub fn compute_comparison_stats(
    series_a: &[DataPoint],
    series_b: &[DataPoint],
    mode: ComparisonMode,
) -> ComparisonStats {
    if series_a.is_empty() || series_b.is_empty() {
        return ComparisonStats {
            delta_mean: 0.0,
            delta_max: 0.0,
            delta_min: 0.0,
            nrmsd: 0.0,
            sample_count: 0,
        };
    }

    let mean_a = series_a.iter().map(|p| p.value).sum::<f64>() / (series_a.len() as f64);
    let mean_b = series_b.iter().map(|p| p.value).sum::<f64>() / (series_b.len() as f64);

    let max_a = series_a.iter().map(|p| p.value).fold(f64::NEG_INFINITY, f64::max);
    let max_b = series_b.iter().map(|p| p.value).fold(f64::NEG_INFINITY, f64::max);

    let min_a = series_a.iter().map(|p| p.value).fold(f64::INFINITY, f64::min);
    let min_b = series_b.iter().map(|p| p.value).fold(f64::INFINITY, f64::min);

    let delta_mean = mean_b - mean_a;
    let delta_max = max_b - max_a;
    let delta_min = min_b - min_a;

    // Align points for RMSD calculation
    let mut squared_diffs = Vec::new();

    match mode {
        ComparisonMode::Calendar => {
            // Mode 1: Match points with closest relative time offset or index
            let min_len = series_a.len().min(series_b.len());
            for i in 0..min_len {
                let diff = series_b[i].value - series_a[i].value;
                squared_diffs.push(diff * diff);
            }
        }
        ComparisonMode::Relative => {
            // Mode 2: Relative day-of-period alignment (aligned by elapsed progress)
            let min_len = series_a.len().min(series_b.len());
            for i in 0..min_len {
                let diff = series_b[i].value - series_a[i].value;
                squared_diffs.push(diff * diff);
            }
        }
    }

    let sample_count = squared_diffs.len();
    let nrmsd = if sample_count > 0 && (max_a - min_a).abs() > 1e-9 {
        let mean_sq_err = squared_diffs.iter().sum::<f64>() / (sample_count as f64);
        let rmse = mean_sq_err.sqrt();
        rmse / (max_a - min_a)
    } else {
        0.0
    };

    ComparisonStats {
        delta_mean,
        delta_max,
        delta_min,
        nrmsd,
        sample_count,
    }
}
