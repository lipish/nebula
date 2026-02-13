use std::sync::Arc;

use dashmap::DashMap;
use nebula_common::{EndpointInfo, EndpointStats, EndpointStatus, ExecutionContext};

pub mod strategy;

use strategy::{Candidate, LeastPending, RoutingStrategy};

/// KV cache usage threshold (fraction 0.0–1.0). When ALL endpoints exceed this,
/// admission control kicks in and returns Overloaded.
const KV_CACHE_OVERLOAD_THRESHOLD: f64 = 0.95;

#[derive(Debug, Clone)]
pub enum RouteError {
    /// No ready endpoint found for the requested model.
    NoEndpoint,
    /// All endpoints are overloaded (kv_cache_usage > threshold).
    Overloaded,
}

impl std::fmt::Display for RouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteError::NoEndpoint => write!(f, "no ready endpoint"),
            RouteError::Overloaded => write!(f, "all endpoints overloaded"),
        }
    }
}

pub struct Router {
    endpoints: DashMap<(String, u32), EndpointInfo>,
    stats: DashMap<(String, u32), EndpointStats>,
    session_affinity: DashMap<String, (String, u32)>,
    strategy: Box<dyn RoutingStrategy>,
    /// model_name → model_uid (e.g. "Qwen/Qwen2.5-0.5B-Instruct" → "qwen2_5_0_5b")
    model_names: DashMap<String, String>,
    /// model_uid → model_name (reverse mapping)
    model_uids_to_names: DashMap<String, String>,
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Router")
            .field("strategy", &self.strategy.name())
            .finish()
    }
}

impl Router {
    pub fn new() -> Arc<Self> {
        Self::with_strategy(Box::new(LeastPending))
    }

    pub fn with_strategy(strategy: Box<dyn RoutingStrategy>) -> Arc<Self> {
        tracing::info!(strategy = strategy.name(), "router initialized");
        Arc::new(Self {
            endpoints: DashMap::new(),
            stats: DashMap::new(),
            session_affinity: DashMap::new(),
            strategy,
            model_names: DashMap::new(),
            model_uids_to_names: DashMap::new(),
        })
    }

    pub fn strategy_name(&self) -> &'static str {
        self.strategy.name()
    }

    pub fn replace_all_endpoints(&self, infos: Vec<EndpointInfo>) {
        self.endpoints.clear();
        for info in infos {
            self.endpoints
                .insert((info.model_uid.clone(), info.replica_id), info);
        }
        self.session_affinity.clear();
    }

    pub fn upsert_endpoint(&self, info: EndpointInfo) {
        self.endpoints
            .insert((info.model_uid.clone(), info.replica_id), info);
    }

    pub fn remove_endpoint(&self, model_uid: &str, replica_id: u32) {
        self.endpoints.remove(&(model_uid.to_string(), replica_id));
    }

    pub fn upsert_stats(&self, stats: EndpointStats) {
        self.stats
            .insert((stats.model_uid.clone(), stats.replica_id), stats);
    }

    pub fn clear_session_affinity(&self, session_id: &str) {
        self.session_affinity.remove(session_id);
    }

    /// Register a bidirectional model_uid ↔ model_name mapping.
    pub fn set_model_mapping(&self, model_uid: &str, model_name: &str) {
        self.model_names
            .insert(model_name.to_string(), model_uid.to_string());
        self.model_uids_to_names
            .insert(model_uid.to_string(), model_name.to_string());
    }

    /// Resolve an input model string to a model_uid.
    /// If `input` is already a known model_uid, return it as-is.
    /// Otherwise, check if it's a known model_name and return the mapped model_uid.
    /// Falls back to returning the input unchanged.
    pub fn resolve_model(&self, input: &str) -> String {
        // Already a known model_uid?
        if self.model_uids_to_names.contains_key(input) {
            return input.to_string();
        }
        // Known model_name → model_uid?
        if let Some(uid) = self.model_names.get(input) {
            return uid.value().clone();
        }
        // Fallback: return as-is
        input.to_string()
    }

    /// Get the user-facing model_name for a given model_uid.
    pub fn get_model_name(&self, model_uid: &str) -> Option<String> {
        self.model_uids_to_names.get(model_uid).map(|v| v.value().clone())
    }

    /// Collect all stats as a slice snapshot (for admission control, etc.).
    pub fn all_stats_for_model(&self, model_uid: &str) -> Vec<EndpointStats> {
        self.stats
            .iter()
            .filter(|e| e.key().0 == model_uid)
            .map(|e| e.value().clone())
            .collect()
    }

    /// Check if all endpoints for a model are overloaded based on KV cache usage.
    fn check_overloaded(&self, stats_snapshot: &[Option<EndpointStats>]) -> bool {
        // Need at least one endpoint with KV cache data to make a judgment
        let mut has_kv_data = false;
        let mut all_overloaded = true;

        for s in stats_snapshot.iter().flatten() {
            if let (Some(used), Some(free)) = (s.kv_cache_used_bytes, s.kv_cache_free_bytes) {
                has_kv_data = true;
                let total = used + free;
                if total > 0 {
                    let usage = used as f64 / total as f64;
                    if usage < KV_CACHE_OVERLOAD_THRESHOLD {
                        all_overloaded = false;
                        break;
                    }
                } else {
                    all_overloaded = false;
                    break;
                }
            } else {
                // No KV data for this endpoint — can't confirm overload
                all_overloaded = false;
                break;
            }
        }

        has_kv_data && all_overloaded
    }

    pub fn route_with_plan_version(
        &self,
        ctx: &ExecutionContext,
        model_uid: &str,
        plan_version: u64,
    ) -> Result<EndpointInfo, RouteError> {
        // Session affinity check
        if let Some(session_id) = ctx.session_id.as_deref() {
            if let Some(aff) = self.session_affinity.get(session_id) {
                let (aff_model_uid, aff_replica_id) = aff.value();
                if aff_model_uid == model_uid {
                    if let Some(ep) = self
                        .endpoints
                        .get(&(aff_model_uid.clone(), *aff_replica_id))
                        .map(|v| v.value().clone())
                    {
                        if ep.status == EndpointStatus::Ready && ep.plan_version == plan_version {
                            return Ok(ep);
                        }
                    }
                }
            }
        }

        // Build candidate list: filter by model_uid, Ready, plan_version
        let filtered: Vec<EndpointInfo> = self
            .endpoints
            .iter()
            .filter(|e| {
                let ep = e.value();
                ep.model_uid == model_uid
                    && ep.status == EndpointStatus::Ready
                    && ep.plan_version == plan_version
            })
            .map(|e| e.value().clone())
            .collect();

        if filtered.is_empty() {
            return Err(RouteError::NoEndpoint);
        }

        let stats_snapshot: Vec<Option<EndpointStats>> = filtered
            .iter()
            .map(|ep| {
                self.stats
                    .get(&(ep.model_uid.clone(), ep.replica_id))
                    .map(|s| s.value().clone())
            })
            .collect();

        // Admission control: reject if all endpoints are overloaded
        if self.check_overloaded(&stats_snapshot) {
            return Err(RouteError::Overloaded);
        }

        let candidates: Vec<Candidate> = filtered
            .iter()
            .zip(stats_snapshot.iter())
            .map(|(ep, s)| Candidate {
                endpoint: ep,
                stats: s.as_ref(),
            })
            .collect();

        let selected = self
            .strategy
            .select(&candidates)
            .map(|i| filtered[i].clone())
            .ok_or(RouteError::NoEndpoint)?;

        if let Some(session_id) = ctx.session_id.clone() {
            self.session_affinity
                .insert(session_id, (selected.model_uid.clone(), selected.replica_id));
        }

        Ok(selected)
    }

    pub fn route(&self, ctx: &ExecutionContext, model_uid: &str) -> Result<EndpointInfo, RouteError> {
        // Session affinity check
        if let Some(session_id) = ctx.session_id.as_deref() {
            if let Some(aff) = self.session_affinity.get(session_id) {
                let (aff_model_uid, aff_replica_id) = aff.value();
                if aff_model_uid == model_uid {
                    if let Some(ep) = self
                        .endpoints
                        .get(&(aff_model_uid.clone(), *aff_replica_id))
                        .map(|v| v.value().clone())
                    {
                        if ep.status == EndpointStatus::Ready {
                            return Ok(ep);
                        }
                    }
                }
            }
        }

        // Build candidate list: filter by model_uid, Ready
        let filtered: Vec<EndpointInfo> = self
            .endpoints
            .iter()
            .filter(|e| {
                let ep = e.value();
                ep.model_uid == model_uid && ep.status == EndpointStatus::Ready
            })
            .map(|e| e.value().clone())
            .collect();

        if filtered.is_empty() {
            return Err(RouteError::NoEndpoint);
        }

        let stats_snapshot: Vec<Option<EndpointStats>> = filtered
            .iter()
            .map(|ep| {
                self.stats
                    .get(&(ep.model_uid.clone(), ep.replica_id))
                    .map(|s| s.value().clone())
            })
            .collect();

        // Admission control: reject if all endpoints are overloaded
        if self.check_overloaded(&stats_snapshot) {
            return Err(RouteError::Overloaded);
        }

        let candidates: Vec<Candidate> = filtered
            .iter()
            .zip(stats_snapshot.iter())
            .map(|(ep, s)| Candidate {
                endpoint: ep,
                stats: s.as_ref(),
            })
            .collect();

        let selected = self
            .strategy
            .select(&candidates)
            .map(|i| filtered[i].clone())
            .ok_or(RouteError::NoEndpoint)?;

        if let Some(session_id) = ctx.session_id.clone() {
            self.session_affinity
                .insert(session_id, (selected.model_uid.clone(), selected.replica_id));
        }

        Ok(selected)
    }
}
