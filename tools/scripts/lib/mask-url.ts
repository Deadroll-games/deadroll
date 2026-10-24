// RPC URLs carry secrets (api-key in the query, a token in the path, user:pass). Nothing that
// reaches stdout or stderr may hold one: every print of a URL, a command line or an error goes
// through here.

/** Keeps the scheme and the host, replaces credentials, every path segment and every query value with `***`. */
export function maskUrl(raw: string): string {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return "***";
  }
  const auth = url.username || url.password ? "***@" : "";
  const path = url.pathname.split("/").filter(Boolean).length > 0 ? "/***" : "";
  const keys = [...new Set([...url.searchParams.keys()])];
  const query = keys.length > 0 ? "?" + keys.map((k) => `${encodeURIComponent(k)}=***`).join("&") : "";
  return `${url.protocol}//${auth}${url.host}${path}${query}`;
}

/** Masks every http(s) and ws(s) URL inside free text: command lines, error messages. */
export function maskText(text: string): string {
  return text.replace(/\b(?:https?|wss?):\/\/[^\s"'`<>)]+/gi, (m) => maskUrl(m));
}
