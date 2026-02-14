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
  ModelView,
  ModelDetailView,
  ModelTemplate,
  DiskAlert,
} from '@/lib/types'

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
}
