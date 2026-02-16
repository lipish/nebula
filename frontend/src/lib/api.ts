const BASE_URL = import.meta.env.VITE_BFF_BASE_URL || '/api'

function buildHeaders(token?: string, json = true) {
  const headers: Record<string, string> = {}
  if (json) {
    headers['Content-Type'] = 'application/json'
  }
  if (token) {
    headers.Authorization = `Bearer ${token}`
  }
  return headers
}

export async function apiGet<T>(path: string, token?: string): Promise<T> {
  const resp = await fetch(`${BASE_URL}${path}`, {
    headers: buildHeaders(token, false),
  })

  if (!resp.ok) {
    const text = await resp.text()
    throw new Error(text || `Request failed: ${resp.status}`)
  }

  return (await resp.json()) as T
}

export async function apiPost<T, Body>(
  path: string,
  body: Body,
  token?: string
): Promise<T> {
  const resp = await fetch(`${BASE_URL}${path}`, {
    method: 'POST',
    headers: buildHeaders(token),
    body: JSON.stringify(body),
  })

  if (!resp.ok) {
    const text = await resp.text()
    throw new Error(text || `Request failed: ${resp.status}`)
  }

  return (await resp.json()) as T
}

export async function apiGetWithParams<T>(
  path: string,
  params: Record<string, string>,
  token?: string
): Promise<T> {
  const query = new URLSearchParams(params).toString()
  const resp = await fetch(`${BASE_URL}${path}?${query}`, {
    headers: buildHeaders(token, false),
  })

  if (!resp.ok) {
    const text = await resp.text()
    throw new Error(text || `Request failed: ${resp.status}`)
  }

  return (await resp.json()) as T
}

export async function apiPut<T, Body>(
  path: string,
  body: Body,
  token?: string
): Promise<T> {
  const resp = await fetch(`${BASE_URL}${path}`, {
    method: 'PUT',
    headers: buildHeaders(token),
    body: JSON.stringify(body),
  })

  if (!resp.ok) {
    const text = await resp.text()
    throw new Error(text || `Request failed: ${resp.status}`)
  }

  return (await resp.json()) as T
}

export async function apiDelete<T>(path: string, token?: string): Promise<T> {
  const resp = await fetch(`${BASE_URL}${path}`, {
    method: 'DELETE',
    headers: buildHeaders(token, false),
  })

  if (!resp.ok) {
    const text = await resp.text()
    throw new Error(text || `Request failed: ${resp.status}`)
  }

  return (await resp.json()) as T
}

// ---------------------------------------------------------------------------
// v2 API convenience functions
// ---------------------------------------------------------------------------
// The v2 routes are mounted at /api/v2 on the BFF. Since BASE_URL is /api,
// we simply prefix paths with /v2 so the final URL becomes /api/v2/...

import type {
  AuthUser,
  CreateUserPayload,
  GatewayLatency,
  GatewayOverview,
  GatewayProtection,
  GatewayReliability,
  GatewayTraffic,
  ManagedUser,
  ModelView,
  ModelDetailView,
  ModelTemplate,
  DiskAlert,
  LoginResponse,
  UpdateUserPayload,
  UserSettings,
} from '@/lib/types'

export const authApi = {
  login: (username: string, password: string) =>
    apiPost<LoginResponse, { username: string; password: string }>('/auth/login', { username, password }),

  logout: (token?: string) =>
    apiPost<{ ok: boolean }, Record<string, never>>('/auth/logout', {}, token),

  me: (token?: string) =>
    apiGet<AuthUser>('/auth/me', token),

  updateProfile: (body: { display_name?: string; email?: string }, token?: string) =>
    apiPut<{ ok: boolean }, { display_name?: string; email?: string }>('/auth/profile', body, token),

  getSettings: (token?: string) =>
    apiGet<UserSettings>('/auth/settings', token),

  updateSettings: (body: Partial<UserSettings>, token?: string) =>
    apiPut<{ ok: boolean }, Partial<UserSettings>>('/auth/settings', body, token),

  listUsers: (token?: string) =>
    apiGet<ManagedUser[]>('/auth/users', token),

  createUser: (body: CreateUserPayload, token?: string) =>
    apiPost<{ ok: boolean; id: string }, CreateUserPayload>('/auth/users', body, token),

  updateUser: (id: string, body: UpdateUserPayload, token?: string) =>
    apiPut<{ ok: boolean }, UpdateUserPayload>(`/auth/users/${id}`, body, token),

  deleteUser: (id: string, token?: string) =>
    apiDelete<{ ok: boolean }>(`/auth/users/${id}`, token),
}

export const v2 = {
  listModels: (token?: string) =>
    apiGet<ModelView[]>('/v2/models', token),

  getModel: (uid: string, token?: string) =>
    apiGet<ModelDetailView>(`/v2/models/${uid}`, token),

  createModel: (body: Record<string, unknown>, token?: string) =>
    apiPost<unknown, Record<string, unknown>>('/v2/models', body, token),

  updateModel: (uid: string, body: Record<string, unknown>, token?: string) =>
    apiPut<unknown, Record<string, unknown>>(`/v2/models/${uid}`, body, token),

  startModel: (uid: string, body?: Record<string, unknown>, token?: string) =>
    apiPost<unknown, Record<string, unknown>>(`/v2/models/${uid}/start`, body || {}, token),

  stopModel: (uid: string, token?: string) =>
    apiPost<unknown, Record<string, unknown>>(`/v2/models/${uid}/stop`, {}, token),

  deleteModel: (uid: string, token?: string) =>
    apiDelete<unknown>(`/v2/models/${uid}`, token),

  scaleModel: (uid: string, replicas: number, token?: string) =>
    apiPut<unknown, { replicas: number }>(`/v2/models/${uid}/scale`, { replicas }, token),

  listTemplates: (token?: string) =>
    apiGet<ModelTemplate[]>('/v2/templates', token),

  deployTemplate: (id: string, body: Record<string, unknown>, token?: string) =>
    apiPost<unknown, Record<string, unknown>>(`/v2/templates/${id}/deploy`, body, token),

  listAlerts: (token?: string) =>
    apiGet<DiskAlert[]>('/v2/alerts', token),

  gatewayOverview: (window: string, token?: string) =>
    apiGetWithParams<GatewayOverview>('/v2/observability/gateway/overview', { window }, token),

  gatewayTraffic: (window: string, token?: string) =>
    apiGetWithParams<GatewayTraffic>('/v2/observability/gateway/traffic', { window }, token),

  gatewayReliability: (window: string, token?: string) =>
    apiGetWithParams<GatewayReliability>('/v2/observability/gateway/reliability', { window }, token),

  gatewayProtection: (window: string, token?: string) =>
    apiGetWithParams<GatewayProtection>('/v2/observability/gateway/protection', { window }, token),

  gatewayLatency: (window: string, token?: string) =>
    apiGetWithParams<GatewayLatency>('/v2/observability/gateway/latency', { window }, token),
}
