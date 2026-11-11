export interface HealthResponse {
  status: string;
  port: number;
  version: string;
}

export interface Observation {
  id?: number;
  timestamp: number;
  layer: string;
  provider: string;
  entity_id?: string;
  latitude?: number;
  longitude?: number;
  altitude_m?: number;
  metric_name?: string;
  metric_value?: number;
  attributes?: Record<string, any>;
  is_interpolated: boolean;
}

/**
 * Fetches server health status and active port from Axum backend.
 */
export async function fetchHealth(): Promise<HealthResponse> {
  try {
    const res = await fetch('/api/v1/health');
    if (!res.ok) {
      throw new Error(`Server returned HTTP ${res.status}`);
    }
    return await res.json();
  } catch (err) {
    return {
      status: 'offline',
      port: 8374,
      version: '0.1.0',
    };
  }
}

/**
 * Queries latest observations by layer from the backend SQLite database.
 */
export async function fetchObservations(layer?: string, limit: number = 100): Promise<Observation[]> {
  try {
    const url = layer ? `/api/v1/observations?layer=${layer}&limit=${limit}` : `/api/v1/observations?limit=${limit}`;
    const res = await fetch(url);
    if (!res.ok) {
      throw new Error(`Failed to fetch observations: ${res.status}`);
    }
    return await res.json();
  } catch (err) {
    console.warn(`[HUMBOLDT] Failed to fetch ${layer ?? 'all'} observations:`, err);
    return [];
  }
}

export interface ResolvedHierarchy {
  country?: string;
  country_code?: string;
  state?: string;
  state_code?: string;
  county?: string;
  city?: string;
  distance_to_city_km?: number;
}

export interface NewsArticle {
  title: string;
  published_at: number;
  source: string;
  url: string;
  provenance: {
    type: 'Direct' | 'Regional' | 'Statewide' | 'National';
    entity: string;
  };
}

/**
 * Resolves WGS84 coordinates into an administrative hierarchy (< 5ms offline).
 */
export async function resolveGeo(lat: number, lon: number, targetLevel?: string): Promise<ResolvedHierarchy> {
  try {
    const url = targetLevel
      ? `/api/v1/geo/resolve?lat=${lat}&lon=${lon}&target_level=${encodeURIComponent(targetLevel)}`
      : `/api/v1/geo/resolve?lat=${lat}&lon=${lon}`;
    const res = await fetch(url);
    if (!res.ok) {
      throw new Error(`Geo resolve failed with HTTP ${res.status}`);
    }
    return await res.json();
  } catch (err) {
    console.warn('[HUMBOLDT] Failed to resolve geo hierarchy:', err);
    return {};
  }
}

/**
 * Fetches news headlines for a geographic area with hierarchical fallback.
 */
export async function fetchNews(
  lat?: number,
  lon?: number,
  city?: string,
  state?: string,
  country?: string,
  targetLevel?: string
): Promise<NewsArticle[]> {
  try {
    const params = new URLSearchParams();
    if (lat !== undefined && lon !== undefined) {
      params.append('lat', lat.toString());
      params.append('lon', lon.toString());
    }
    if (city) params.append('city', city);
    if (state) params.append('state', state);
    if (country) params.append('country', country);
    if (targetLevel) params.append('target_level', targetLevel);

    const res = await fetch(`/api/v1/news?${params.toString()}`);
    if (!res.ok) {
      throw new Error(`News query failed with HTTP ${res.status}`);
    }
    return await res.json();
  } catch (err) {
    console.warn('[HUMBOLDT] Failed to fetch hierarchical news:', err);
    return [];
  }
}

export interface DataPoint {
  timestamp: number;
  value: number;
}

export interface SnapshotSummary {
  id: string;
  name: string;
  created_at: number;
  layer: string;
  metric_name: string;
  query_entity?: string;
  start_time: number;
  end_time: number;
  resolution: string;
  metadata_json: string;
  point_count: number;
}

export interface SnapshotDetail extends SnapshotSummary {
  data_points: DataPoint[];
}

export interface ComparisonStats {
  delta_mean: number;
  delta_max: number;
  delta_min: number;
  nrmsd: number;
  sample_count: number;
}

export interface CompareSnapshotsResponse {
  snapshot_a_id: string;
  snapshot_a_name: string;
  snapshot_b_id: string;
  snapshot_b_name: string;
  mode: 'calendar' | 'relative';
  stats: ComparisonStats;
  series_a: DataPoint[];
  series_b: DataPoint[];
}

/**
 * Queries time-series data points from observations and hourly rollups.
 */
export async function fetchMetricSeries(
  layer: string,
  metricName?: string,
  entityId?: string,
  from?: number,
  to?: number
): Promise<DataPoint[]> {
  try {
    const params = new URLSearchParams({ layer });
    if (metricName) params.append('metric_name', metricName);
    if (entityId) params.append('entity_id', entityId);
    if (from !== undefined) params.append('from', from.toString());
    if (to !== undefined) params.append('to', to.toString());

    const res = await fetch(`/api/v1/metrics/series?${params.toString()}`);
    if (!res.ok) {
      throw new Error(`Failed to fetch metric series: ${res.status}`);
    }
    return await res.json();
  } catch (err) {
    console.warn('[HUMBOLDT] Failed to fetch metric series:', err);
    return [];
  }
}

/**
 * Freezes a metric series dataset into a permanent analytical snapshot.
 */
export async function saveSnapshot(payload: {
  name: string;
  layer: string;
  metric_name: string;
  query_entity?: string;
  start_time?: number;
  end_time?: number;
  resolution?: string;
  metadata_json?: string;
  data_points: DataPoint[];
}): Promise<SnapshotSummary> {
  const res = await fetch('/api/v1/snapshots', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    throw new Error(`Failed to save snapshot: ${res.status}`);
  }
  return await res.json();
}

/**
 * Lists all user-saved analytical snapshots.
 */
export async function listSnapshots(): Promise<SnapshotSummary[]> {
  try {
    const res = await fetch('/api/v1/snapshots');
    if (!res.ok) {
      throw new Error(`Failed to list snapshots: ${res.status}`);
    }
    return await res.json();
  } catch (err) {
    console.warn('[HUMBOLDT] Failed to list snapshots:', err);
    return [];
  }
}

/**
 * Retrieves a single snapshot with decoded data points.
 */
export async function getSnapshot(id: string): Promise<SnapshotDetail> {
  const res = await fetch(`/api/v1/snapshots/${id}`);
  if (!res.ok) {
    throw new Error(`Failed to fetch snapshot ${id}: ${res.status}`);
  }
  return await res.json();
}

/**
 * Deletes a snapshot by UUID.
 */
export async function deleteSnapshot(id: string): Promise<boolean> {
  const res = await fetch(`/api/v1/snapshots/${id}`, { method: 'DELETE' });
  if (!res.ok) {
    throw new Error(`Failed to delete snapshot ${id}: ${res.status}`);
  }
  const data = await res.json();
  return !!data.deleted;
}

/**
 * Performs overlaid comparison between Snapshot A and Snapshot B.
 */
export async function compareSnapshots(
  snapshotAId: string,
  snapshotBId: string,
  mode: 'calendar' | 'relative' = 'calendar'
): Promise<CompareSnapshotsResponse> {
  const res = await fetch('/api/v1/snapshots/compare', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      snapshot_a_id: snapshotAId,
      snapshot_b_id: snapshotBId,
      mode,
    }),
  });
  if (!res.ok) {
    throw new Error(`Snapshot comparison failed: ${res.status}`);
  }
  return await res.json();
}

/**
 * Returns RFC 4180 CSV export download URL.
 */
export function getSnapshotCsvUrl(id: string): string {
  return `/api/v1/snapshots/${id}/csv`;
}

export interface AuthStatus {
  lan_enabled: boolean;
  password_configured: boolean;
  authenticated: boolean;
}

export interface StorageStatus {
  database_size_bytes: number;
  disk_cap_bytes: number;
  disk_usage_percent: number;
}

export interface AlertRule {
  id: string;
  name: string;
  enabled: boolean;
  layer: string;
  metric_name: string;
  operator: 'greater_than' | 'less_than' | 'equal';
  threshold: number;
  cooldown_seconds: number;
  notify_desktop: boolean;
  notify_discord: boolean;
}

export async function fetchAuthStatus(): Promise<AuthStatus> {
  const res = await fetch('/api/v1/auth/status');
  if (!res.ok) throw new Error('Failed to fetch auth status');
  return await res.json();
}

export async function setLanPassword(password: string): Promise<void> {
  const res = await fetch('/api/v1/auth/password', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ password }),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Password update failed' }));
    throw new Error(err.error || 'Password update failed');
  }
}

export async function setLanMode(enabled: boolean): Promise<void> {
  const res = await fetch('/api/v1/auth/lan', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ enabled }),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Failed to update LAN mode' }));
    throw new Error(err.error || 'Failed to update LAN mode');
  }
}

export async function loginLan(password: string): Promise<void> {
  const res = await fetch('/api/v1/auth/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ password }),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Login failed' }));
    throw new Error(err.error || 'Login failed');
  }
}

export async function logoutLan(): Promise<void> {
  await fetch('/api/v1/auth/logout', { method: 'POST' });
}

export async function fetchStorageStatus(): Promise<StorageStatus> {
  const res = await fetch('/api/v1/storage/status');
  if (!res.ok) throw new Error('Failed to fetch storage status');
  return await res.json();
}

export async function clearCaches(): Promise<{ cleared_records: number }> {
  const res = await fetch('/api/v1/storage/clear-cache', { method: 'POST' });
  if (!res.ok) throw new Error('Failed to clear caches');
  return await res.json();
}

export async function deleteAllSnapshots(): Promise<{ deleted_snapshots: number }> {
  const res = await fetch('/api/v1/storage/delete-snapshots', { method: 'POST' });
  if (!res.ok) throw new Error('Failed to delete snapshots');
  return await res.json();
}

export async function compactStorage(): Promise<void> {
  const res = await fetch('/api/v1/storage/compact', { method: 'POST' });
  if (!res.ok) throw new Error('Database compaction failed');
}

export async function fetchAlertRules(): Promise<AlertRule[]> {
  const res = await fetch('/api/v1/alerts/rules');
  if (!res.ok) throw new Error('Failed to fetch alert rules');
  return await res.json();
}

export async function saveAlertRule(rule: AlertRule): Promise<void> {
  const res = await fetch('/api/v1/alerts/rules', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(rule),
  });
  if (!res.ok) throw new Error('Failed to save alert rule');
}

export async function deleteAlertRule(id: string): Promise<void> {
  const res = await fetch(`/api/v1/alerts/rules/${id}`, { method: 'DELETE' });
  if (!res.ok) throw new Error('Failed to delete alert rule');
}

export async function testToast(title?: string, message?: string): Promise<void> {
  const res = await fetch('/api/v1/alerts/test-toast', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ title, message }),
  });
  if (!res.ok) throw new Error('Desktop toast failed');
}

export async function testDiscordWebhook(webhookUrl?: string): Promise<void> {
  const res = await fetch('/api/v1/alerts/test-discord', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ webhook_url: webhookUrl }),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Discord webhook test failed' }));
    throw new Error(err.error || 'Discord webhook test failed');
  }
}

export async function setVaultSecret(keyName: string, secretValue: string): Promise<void> {
  const res = await fetch('/api/v1/vault', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ key_name: keyName, secret_value: secretValue }),
  });
  if (!res.ok) throw new Error('Failed to save secret to vault');
}

export async function fetchVaultKeys(): Promise<string[]> {
  const res = await fetch('/api/v1/vault/keys');
  if (!res.ok) throw new Error('Failed to fetch vault keys');
  return await res.json();
}

export interface ProviderAttribution {
  id: string;
  category: string;
  name: string;
  description: string;
  license: string;
  license_type: string;
  url: string;
}

export async function fetchAttributions(): Promise<ProviderAttribution[]> {
  const res = await fetch('/api/v1/attribution');
  if (!res.ok) throw new Error('Failed to fetch provider attributions');
  return await res.json();
}

export interface TestKeyResult {
  valid: boolean;
  message: string;
}

export async function testProviderKey(providerId: string, apiKey: string): Promise<TestKeyResult> {
  const res = await fetch('/api/v1/providers/test-key', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ provider_id: providerId, api_key: apiKey }),
  });
  if (!res.ok) throw new Error('Key testing endpoint failed');
  return await res.json();
}

export async function deleteProviderKey(providerId: string): Promise<void> {
  const res = await fetch(`/api/v1/providers/${encodeURIComponent(providerId)}/key`, {
    method: 'DELETE',
  });
  if (!res.ok) throw new Error('Failed to delete provider key');
}

export async function shutdownApp(): Promise<void> {
  const res = await fetch('/api/v1/shutdown', {
    method: 'POST',
  });
  if (!res.ok) throw new Error('Shutdown request failed');
}

export interface GeoPolygon {
  id: number;
  level: number;
  name: string;
  code?: string;
  parent_code?: string;
  min_lon: number;
  min_lat: number;
  max_lon: number;
  max_lat: number;
  rings: [number, number][][];
}

export interface GeoPlace {
  name: string;
  state_code?: string;
  country_code: string;
  lat: number;
  lon: number;
  population: number;
}

export interface BoundariesResponse {
  countries: GeoPolygon[];
  states: GeoPolygon[];
  counties: GeoPolygon[];
  cities?: GeoPolygon[];
  places: GeoPlace[];
}

export async function fetchBoundaries(): Promise<BoundariesResponse> {
  try {
    const res = await fetch('/api/v1/geo/boundaries');
    if (!res.ok) {
      throw new Error(`Failed to fetch boundaries: ${res.status}`);
    }
    return await res.json();
  } catch (err) {
    console.warn('[HUMBOLDT] Failed to fetch boundaries:', err);
    return { countries: [], states: [], counties: [], places: [] };
  }
}

export interface CableLandingPoint {
  id: string;
  name: string;
  country: string;
  lon: number;
  lat: number;
}

export interface SubmarineCable {
  id: string;
  name: string;
  length_km?: number;
  type: string;
  owners: string[];
  landing_points: CableLandingPoint[];
  coordinates: [number, number][];
}

export async function fetchSubmarineCables(): Promise<SubmarineCable[]> {
  try {
    const res = await fetch('/api/v1/geo/submarine-cables');
    if (!res.ok) {
      throw new Error(`Failed to fetch submarine cables: ${res.status}`);
    }
    return await res.json();
  } catch (err) {
    console.warn('[HUMBOLDT] Failed to fetch submarine cables:', err);
    return [];
  }
}



