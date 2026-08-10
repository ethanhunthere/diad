// ---------------------------------------------------------------------------
// DIAD AGENT — thin REST client for /api/v1/*
// ---------------------------------------------------------------------------

export interface ApiError extends Error {
  status: number;
  body: Record<string, unknown>;
}

/**
 * JSON helper for the daemon's REST API. Throws `ApiError` (with `.status`
 * and `.body`) on non-2xx so callers can surface friendly toasts.
 */
export async function api<T = unknown>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api/v1${path}`, {
    headers: { "content-type": "application/json" },
    ...init,
  });
  const body = (await res.json().catch(() => ({}))) as Record<string, unknown>;
  if (!res.ok) {
    const err = new Error(
      typeof body.message === "string" ? body.message : `HTTP ${res.status}`,
    ) as ApiError;
    err.status = res.status;
    err.body = body;
    throw err;
  }
  return body as T;
}

/** Convenience for POST-with-body. */
export function apiPost<T = unknown>(path: string, body: unknown): Promise<T> {
  return api<T>(path, {
    method: "POST",
    body: JSON.stringify(body),
  });
}

/** Convenience for GET with query params. */
export function apiGet<T = unknown>(path: string, params: Record<string, string | number | boolean | undefined> = {}): Promise<T> {
  const qs = Object.entries(params)
    .filter(([, v]) => v !== undefined && v !== "")
    .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`)
    .join("&");
  return api<T>(qs ? `${path}?${qs}` : path);
}
