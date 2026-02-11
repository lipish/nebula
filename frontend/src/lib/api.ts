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
