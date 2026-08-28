// ---------------------------------------------------------------------------
// DIAD AGENT — thin REST client for /api/v1/*
// ---------------------------------------------------------------------------

export interface ApiError extends Error {
  status: number;
  body: Record<string, unknown>;
}

/**
 * Bearer token for the daemon control plane. Fetched once from the
 * same-origin /gui-token bootstrap endpoint (guarded by the daemon's
 * origin/host checks) and reused for every REST call and the WS URL.
 */
let cachedToken: string | null = null;
let tokenPromise: Promise<string | null> | null = null;

/** Fetch /gui-token with a small retry/backoff: a page can load while the
 * daemon is still writing the token (503), and a one-shot failure must not
 * degrade into permanent unauthenticated requests. */
async function fetchGuiToken(attempt = 0): Promise<string | null> {
  try {
    const res = await fetch("/gui-token", { cache: "no-store" });
    if (res.ok) {
      const body = (await res.json()) as { token?: string };
      if (body?.token) return body.token;
    }
  } catch {
    /* transient fetch error: fall through to retry */
  }
  if (attempt < 3) {
    await new Promise((r) => setTimeout(r, 300 * (attempt + 1)));
    return fetchGuiToken(attempt + 1);
  }
  return null;
}

export function apiToken(): Promise<string | null> {
  if (cachedToken) return Promise.resolve(cachedToken);
  if (!tokenPromise) {
    tokenPromise = fetchGuiToken().then((t) => {
      cachedToken = t;
      return cachedToken;
    });
  }
  return tokenPromise;
}

/** Synchronous best-effort accessor (null before bootstrap completes). */
export function apiTokenSync(): string | null {
  return cachedToken;
}

/**
 * JSON helper for the daemon's REST API. Throws `ApiError` (with `.status`
 * and `.body`) on non-2xx so callers can surface friendly toasts.
 */
export async function api<T = unknown>(path: string, init?: RequestInit): Promise<T> {
  const token = await apiToken();
  const headers: Record<string, string> = { "content-type": "application/json" };
  if (token) headers["authorization"] = `Bearer ${token}`;
  const res = await fetch(`/api/v1${path}`, {
    headers,
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
export function apiGet<T = unknown>(
  path: string,
  params: Record<string, string | number | boolean | undefined> = {},
): Promise<T> {
  const qs = Object.entries(params)
    .filter(([, v]) => v !== undefined && v !== "")
    .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`)
    .join("&");
  return api<T>(qs ? `${path}?${qs}` : path);
}
