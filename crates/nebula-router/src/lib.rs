use std::sync::Arc;

use nebula_common::{EndpointInfo, EndpointStats, EndpointStatus, ExecutionContext};
use dashmap::DashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ReplicaKey<'a> {
    model_uid: &'a str,
    replica_id: u32,
}

#[derive(Debug, Default)]
pub struct Router {
    endpoints: DashMap<(String, u32), EndpointInfo>,
    stats: DashMap<(String, u32), EndpointStats>,
    session_affinity: DashMap<String, (String, u32)>,
}

impl Router {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
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

    pub fn route_with_plan_version(
        &self,
        ctx: &ExecutionContext,
        model_uid: &str,
        plan_version: u64,
    ) -> Option<EndpointInfo> {
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
                            return Some(ep);
                        }
                    }
                }
            }
        }

        let mut best: Option<EndpointInfo> = None;
        let mut best_pending: u64 = u64::MAX;

        for entry in self.endpoints.iter() {
            let ep = entry.value();
            if ep.model_uid != model_uid {
                continue;
            }
            if ep.status != EndpointStatus::Ready {
                continue;
            }
            if ep.plan_version != plan_version {
                continue;
            }

            let pending = self
                .stats
                .get(&(ep.model_uid.clone(), ep.replica_id))
                .map(|s| s.pending_requests)
                .unwrap_or(0);

            if pending < best_pending {
                best_pending = pending;
                best = Some(ep.clone());
            }
        }

        if let (Some(session_id), Some(best_ep)) = (ctx.session_id.clone(), best.as_ref()) {
            self.session_affinity
                .insert(session_id, (best_ep.model_uid.clone(), best_ep.replica_id));
        }

        best
    }

    pub fn route(&self, ctx: &ExecutionContext, model_uid: &str) -> Option<EndpointInfo> {
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
                            return Some(ep);
                        }
                    }
                }
            }
        }

        let mut best: Option<EndpointInfo> = None;
        let mut best_pending: u64 = u64::MAX;

        for entry in self.endpoints.iter() {
            let ep = entry.value();
            if ep.model_uid != model_uid {
                continue;
            }
            if ep.status != EndpointStatus::Ready {
                continue;
            }

            let pending = self
                .stats
                .get(&(ep.model_uid.clone(), ep.replica_id))
                .map(|s| s.pending_requests)
                .unwrap_or(0);

            if pending < best_pending {
                best_pending = pending;
                best = Some(ep.clone());
            }
        }

        if let (Some(session_id), Some(best_ep)) = (ctx.session_id.clone(), best.as_ref()) {
            self.session_affinity
                .insert(session_id, (best_ep.model_uid.clone(), best_ep.replica_id));
        }

        best
    }
}
