export interface GpuStatus {
  index: number
  memory_total_mb: number
  memory_used_mb: number
  temperature_c?: number | null
  utilization_gpu?: number | null
}

export interface NodeStatus {
  node_id: string
  last_heartbeat_ms: number
  gpus: GpuStatus[]
  api_addr?: string | null
}

export interface EndpointInfo {
  model_uid: string
  replica_id: number
  plan_version: number
  node_id: string
  endpoint_kind: string
  api_flavor: string
  status: string
  last_heartbeat_ms: number
  grpc_target?: string | null
  base_url?: string | null
}

export interface PlacementAssignment {
  replica_id: number
  node_id: string
  engine_config_path: string
  port: number
  gpu_index?: number | null
  gpu_indices?: number[] | null
  extra_args?: string[] | null
  engine_type?: string | null
  docker_image?: string | null
}

export interface PlacementPlan {
  request_id?: string | null
  model_uid: string
  model_name?: string
  version: number
  assignments: PlacementAssignment[]
}

export interface ModelConfig {
  tensor_parallel_size?: number | null
  gpu_memory_utilization?: number | null
  max_model_len?: number | null
  required_vram_mb?: number | null
  lora_modules?: string[] | null
}

export interface ModelLoadRequest {
  model_name: string
  model_uid: string
  replicas?: number
  config?: ModelConfig | null
  node_id?: string | null
  gpu_index?: number | null
  gpu_indices?: number[] | null
  engine_type?: string | null
  docker_image?: string | null
}

export interface ModelRequest {
  id: string
  request: ModelLoadRequest
  status: unknown
  created_at_ms: number
}

export interface ModelSearchResult {
  id: string
  name: string
  author: string | null
  downloads: number
  likes: number
  tags: string[]
  pipeline_tag: string | null
  source: string
}

export interface EndpointStats {
  model_uid: string
  replica_id: number
  last_updated_ms: number
  pending_requests: number
  prefix_cache_hit_rate?: number | null
  prompt_cache_hit_rate?: number | null
  kv_cache_used_bytes?: number | null
  kv_cache_free_bytes?: number | null
}

export interface ClusterStatus {
  nodes: NodeStatus[]
  endpoints: EndpointInfo[]
  placements: PlacementPlan[]
  model_requests: ModelRequest[]
}

export interface EngineImage {
  id: string
  engine_type: string
  image: string
  platforms: string[]
  version_policy: 'pin' | 'rolling'
  pre_pull: boolean
  description?: string | null
  created_at_ms: number
  updated_at_ms: number
}

export interface NodeImageStatus {
  node_id: string
  image_id: string
  image: string
  status: 'pending' | 'pulling' | 'ready' | 'failed'
  error?: string | null
  updated_at_ms: number
}

// ---------------------------------------------------------------------------
// v2 API types
// ---------------------------------------------------------------------------

export type AggregatedModelState = 'stopped' | 'downloading' | 'starting' | 'running' | 'degraded' | 'failed' | 'stopping'

export interface ReplicaCount {
  desired: number
  ready: number
  unhealthy: number
}

export interface ModelView {
  model_uid: string
  model_name: string
  engine_type: string | null
  state: AggregatedModelState
  replicas: ReplicaCount
  endpoints: EndpointInfo[]
  labels: Record<string, string>
  created_at_ms: number
  updated_at_ms: number
}

export interface DownloadProgress {
  model_uid: string
  replica_id: number
  node_id: string
  model_name: string
  phase: 'downloading' | 'verifying' | 'complete' | 'failed'
  total_bytes: number
  downloaded_bytes: number
  files_total: number
  files_done: number
  updated_at_ms: number
}

export interface DownloadProgressView {
  replicas: DownloadProgress[]
}

export interface CacheStatusView {
  cached_on_nodes: string[]
  total_size_bytes: number
}

export interface ModelDetailView {
  model_uid: string
  model_name: string
  engine_type: string | null
  state: AggregatedModelState
  replicas: ReplicaCount
  labels: Record<string, string>
  created_at_ms: number
  updated_at_ms: number
  spec: ModelSpec
  deployment: ModelDeployment | null
  placement: PlacementPlan | null
  endpoints: EndpointInfo[]
  stats: EndpointStats[]
  download_progress: DownloadProgressView | null
  cache_status: CacheStatusView | null
}

export interface ModelSpec {
  model_uid: string
  model_name: string
  model_source: 'hugging_face' | 'model_scope' | 'local'
  model_path?: string | null
  engine_type?: string | null
  docker_image?: string | null
  config?: ModelConfig | null
  labels: Record<string, string>
  created_at_ms: number
  updated_at_ms: number
  created_by?: string | null
}

export interface ModelDeployment {
  model_uid: string
  desired_state: 'running' | 'stopped'
  replicas: number
  min_replicas?: number | null
  max_replicas?: number | null
  node_affinity?: string | null
  gpu_affinity?: number[] | null
  config_overrides?: ModelConfig | null
  version: number
  updated_at_ms: number
}

export interface ModelTemplate {
  template_id: string
  name: string
  description?: string | null
  category?: 'llm' | 'embedding' | 'rerank' | 'vlm' | 'audio' | null
  model_name: string
  model_source?: 'hugging_face' | 'model_scope' | 'local' | null
  engine_type?: string | null
  docker_image?: string | null
  config?: ModelConfig | null
  default_replicas: number
  labels: Record<string, string>
  source: 'system' | 'user' | 'saved'
  created_at_ms: number
  updated_at_ms: number
}

export interface ModelCacheEntry {
  node_id: string
  model_name: string
  cache_path: string
  size_bytes: number
  file_count: number
  complete: boolean
  last_accessed_ms: number
  discovered_at_ms: number
}

export interface NodeDiskStatus {
  node_id: string
  model_dir: string
  total_bytes: number
  used_bytes: number
  available_bytes: number
  usage_pct: number
  model_cache_bytes: number
  model_count: number
  updated_at_ms: number
}

export interface DiskAlert {
  node_id: string
  alert_type: 'disk_warning' | 'disk_critical'
  message: string
  model_dir: string
  usage_pct: number
  available_bytes: number
  created_at_ms: number
}
