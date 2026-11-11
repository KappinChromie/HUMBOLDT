use crate::rate_limiter::{CircuitBreaker, TokenBucket};
use humboldt_core::{HumboldtError, Observation, Result, RoutingMode};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

pub type ProviderFetchFn = Box<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<Vec<Observation>>> + Send>>
        + Send
        + Sync,
>;

pub struct ProviderHandle {
    pub slug: String,
    pub token_bucket: TokenBucket,
    pub circuit_breaker: CircuitBreaker,
    pub fetch_fn: ProviderFetchFn,
}

/// Ingestion router supporting Failover, Spread, and Merge modes per Section 6.1.
pub struct ObservationRouter {
    mode: RoutingMode,
    providers: Vec<Arc<ProviderHandle>>,
    spread_counter: AtomicUsize,
}

impl ObservationRouter {
    pub fn new(mode: RoutingMode, providers: Vec<Arc<ProviderHandle>>) -> Self {
        Self {
            mode,
            providers,
            spread_counter: AtomicUsize::new(0),
        }
    }

    /// Executes query routing according to configured mode.
    pub async fn execute(&self) -> Result<Vec<Observation>> {
        match self.mode {
            RoutingMode::Failover => self.execute_failover().await,
            RoutingMode::Spread => self.execute_spread().await,
            RoutingMode::Merge => self.execute_merge().await,
        }
    }

    /// Failover: tries highest-priority available provider; cascades on error.
    async fn execute_failover(&self) -> Result<Vec<Observation>> {
        for provider in &self.providers {
            if !provider.circuit_breaker.can_execute() {
                warn!(
                    "Provider '{}' circuit breaker is OPEN, failing over...",
                    provider.slug
                );
                continue;
            }

            if !provider.token_bucket.try_acquire() {
                warn!(
                    "Provider '{}' rate limit exceeded, failing over...",
                    provider.slug
                );
                continue;
            }

            match (provider.fetch_fn)().await {
                Ok(obs) => {
                    provider.circuit_breaker.record_success();
                    return Ok(obs);
                }
                Err(err) => {
                    warn!(
                        "Provider '{}' failed: {}. Cascading to fallback...",
                        provider.slug, err
                    );
                    provider.circuit_breaker.record_failure();
                }
            }
        }

        Err(HumboldtError::Storage(
            "All failover providers exhausted or unavailable".to_string(),
        ))
    }

    /// Spread: round-robins across healthy providers to distribute API quota.
    async fn execute_spread(&self) -> Result<Vec<Observation>> {
        let count = self.providers.len();
        if count == 0 {
            return Ok(Vec::new());
        }

        let start_idx = self.spread_counter.fetch_add(1, Ordering::SeqCst) % count;

        for i in 0..count {
            let idx = (start_idx + i) % count;
            let provider = &self.providers[idx];

            if !provider.circuit_breaker.can_execute() || !provider.token_bucket.try_acquire() {
                continue;
            }

            match (provider.fetch_fn)().await {
                Ok(obs) => {
                    provider.circuit_breaker.record_success();
                    return Ok(obs);
                }
                Err(err) => {
                    warn!(
                        "Spread provider '{}' failed: {}. Trying next...",
                        provider.slug, err
                    );
                    provider.circuit_breaker.record_failure();
                }
            }
        }

        Err(HumboldtError::Storage(
            "All spread providers failed or rate-limited".to_string(),
        ))
    }

    /// Merge: queries all healthy providers simultaneously and deduplicates by entity_id.
    async fn execute_merge(&self) -> Result<Vec<Observation>> {
        let mut futures = Vec::new();

        for provider in &self.providers {
            if !provider.circuit_breaker.can_execute() || !provider.token_bucket.try_acquire() {
                continue;
            }
            let p = Arc::clone(provider);
            futures.push(async move {
                let res = (p.fetch_fn)().await;
                (p, res)
            });
        }

        let results = futures::future::join_all(futures).await;
        let mut merged_map: HashMap<String, Observation> = HashMap::new();
        let mut unkeyed_records: Vec<Observation> = Vec::new();

        for (provider, res) in results {
            match res {
                Ok(observations) => {
                    provider.circuit_breaker.record_success();
                    for obs in observations {
                        if let Some(ref entity_id) = obs.entity_id {
                            // If duplicate, prefer the freshest timestamp
                            merged_map
                                .entry(entity_id.clone())
                                .and_modify(|existing| {
                                    if obs.timestamp > existing.timestamp {
                                        *existing = obs.clone();
                                    }
                                })
                                .or_insert(obs);
                        } else {
                            unkeyed_records.push(obs);
                        }
                    }
                }
                Err(err) => {
                    warn!("Merge provider '{}' failed: {}", provider.slug, err);
                    provider.circuit_breaker.record_failure();
                }
            }
        }

        let mut final_obs: Vec<Observation> = merged_map.into_values().collect();
        final_obs.extend(unkeyed_records);

        info!("Merge mode unified {} total observations.", final_obs.len());
        Ok(final_obs)
    }
}
