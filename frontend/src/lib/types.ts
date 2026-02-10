export interface GpuStatus {
  index: number
  memory_total_mb: number
  memory_used_mb: number
}

export interface NodeStatus {
  node_id: string
  last_heartbeat_ms: number
  gpus: GpuStatus[]
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
  extra_args?: string[] | null
}

export interface PlacementPlan {
  model_uid: string
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
}

export interface ModelRequest {
  id: string
  request: ModelLoadRequest
  status: string
  created_at_ms: number
}

export interface ClusterStatus {
  nodes: NodeStatus[]
  endpoints: EndpointInfo[]
  placements: PlacementPlan[]
  model_requests: ModelRequest[]
}
