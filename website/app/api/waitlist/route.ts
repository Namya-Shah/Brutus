import { env } from "cloudflare:workers";
export const dynamic = "force-dynamic";
const allowedPlatforms = new Set(["", "macos", "windows", "linux", "multiple"]);
const json = (body: Record<string, unknown>, status = 200) => Response.json(body, { status, headers: { "Cache-Control": "no-store" } });
async function digest(value: string) {
  const bytes = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return Array.from(new Uint8Array(bytes), b => b.toString(16).padStart(2, "0")).join("");
}
function checkRequest(request: Request) {
  const origin = request.headers.get("origin");
  if (origin && origin !== new URL(request.url).origin) return json({ error: "Please submit this form from the Brutus website." }, 403);
  if (!request.headers.get("content-type")?.includes("application/json")) return json({ error: "Please submit a JSON request." }, 415);
  if (Number(request.headers.get("content-length") || 0) > 4096) return json({ error: "Request is too large." }, 413);
  return null;
}
async function readBody(request: Request) {
  const body = await request.text();
  if (body.length > 4096) throw new Error("Request too large");
  const data: unknown = JSON.parse(body);
  if (!data || typeof data !== "object" || Array.isArray(data)) throw new Error("Invalid JSON object");
  return data as Record<string, unknown>;
}
export async function POST(request: Request) {
  const rejected = checkRequest(request);
  if (rejected) return rejected;
  let body: Record<string, unknown>;
  try { body = await readBody(request); }
  catch { return json({ error: "Please check your signup details and try again." }, 400); }
  if (body.website) return json({ error: "We couldn’t verify your signup. Please try again." }, 400);
  const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
  const platform = typeof body.platform === "string" ? body.platform : "";
  if (email.length > 254 || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) return json({ error: "Enter a valid email address." }, 400);
  if (!allowedPlatforms.has(platform)) return json({ error: "Choose a platform from the list." }, 400);
  if (!env.DB) return json({ error: "Signups are temporarily unavailable. Please try again soon." }, 503);
  try {
    const now = Math.floor(Date.now() / 1000);
    const ip = request.headers.get("cf-connecting-ip");
    if (ip) {
      const key = await digest(`brutus-signup:${Math.floor(now / 86400)}:${ip}`);
      const limit = await env.DB.prepare("INSERT INTO signup_limits (key, attempts, expires_at) VALUES (?, 1, ?) ON CONFLICT(key) DO UPDATE SET attempts = attempts + 1 RETURNING attempts").bind(key, now + 86400).first<{ attempts: number }>();
      if (limit && limit.attempts > 15) return json({ error: "Too many signup attempts today. Please try again tomorrow." }, 429);
      await env.DB.prepare("DELETE FROM signup_limits WHERE expires_at < ?").bind(now).run();
    }
    const token = crypto.randomUUID() + crypto.randomUUID();
    const inserted = await env.DB.prepare("INSERT INTO waitlist (id, email, platform, removal_token_hash, created_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(email) DO NOTHING RETURNING id")
      .bind(crypto.randomUUID(), email, platform || null, await digest(token), now).first();
    // Duplicate signups never rotate or disclose the original deletion token.
    return json({ ok: true, ...(inserted ? { removalToken: token } : {}) });
  } catch {
    console.error("Waitlist signup could not be persisted.");
    return json({ error: "We couldn’t save your signup. Please try again in a moment." }, 503);
  }
}
export async function DELETE(request: Request) {
  const rejected = checkRequest(request);
  if (rejected) return rejected;
  let body: Record<string, unknown>;
  try { body = await readBody(request); }
  catch { return json({ error: "Invalid removal request." }, 400); }
  if (typeof body.token !== "string" || !/^[a-f0-9-]{72}$/.test(body.token)) return json({ error: "Invalid removal token." }, 400);
  if (!env.DB) return json({ error: "Please try again later." }, 503);
  try {
    await env.DB.prepare("DELETE FROM waitlist WHERE removal_token_hash = ?").bind(await digest(body.token)).run();
    return json({ ok: true });
  } catch { return json({ error: "Couldn’t remove your signup. Please try again." }, 503); }
}
