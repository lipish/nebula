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
