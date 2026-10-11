// JSON-RPC client for the local LedgerCraft server.

export class ApiError extends Error {
  status: number;
  constructor(message: string, status: number) { super(message); this.status = status; }
}

let token: string | null = null;
try { token = sessionStorage.getItem('lc-session'); } catch {}

export function setToken(t: string | null) {
  token = t;
  try { t ? sessionStorage.setItem('lc-session', t) : sessionStorage.removeItem('lc-session'); } catch {}
}
export function getToken() { return token; }

const listeners: ((e: ApiError) => void)[] = [];
export function onAuthError(fn: (e: ApiError) => void) { listeners.push(fn); }

export async function rpc<T = any>(method: string, params: any = {}): Promise<T> {
  const res = await fetch(`/api/rpc/${method}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', ...(token ? { 'x-session': token } : {}) },
    body: JSON.stringify(params ?? {})
  });
  let body: any = null;
  try { body = await res.json(); } catch {}
  if (!res.ok) {
    const err = new ApiError(body?.error ?? `Request failed (${res.status})`, res.status);
    if (res.status === 401) listeners.forEach((l) => l(err));
    throw err;
  }
  return body as T;
}

/** Download a server-generated file; a server-side error (e.g. missing bank details) is thrown as an ApiError. */
export async function download(path: string) {
  const res = await fetch(path, { headers: token ? { 'x-session': token } : {} });
  if (!res.ok) throw new ApiError((await res.text()) || `Download failed (${res.status})`, res.status);
  const name = /filename="([^"]+)"/.exec(res.headers.get('content-disposition') ?? '')?.[1] ?? 'download';
  const url = URL.createObjectURL(await res.blob());
  const a = document.createElement('a');
  a.href = url; a.download = name; document.body.appendChild(a); a.click(); a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** Open a server-rendered page (invoice print view etc.) in a new window. */
export function openView(path: string) {
  window.open(path + (token ? (path.includes('?') ? '&' : '?') + 's=' + token : ''), '_blank');
}
