import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";

// Run only against a local preview. Test signups are removed before exiting.
const base = process.argv[2] || "http://127.0.0.1:5178";
assert.ok(["127.0.0.1", "localhost"].includes(new URL(base).hostname), "Use a local preview.");
const email = `brutus-check-${randomUUID()}@example.com`;
const call = (body, options = {}) => fetch(`${base}/api/waitlist`, {
  method: "POST",
  ...options,
  headers: { "Content-Type": "application/json", Origin: base, ...options.headers },
  body: JSON.stringify(body),
});
let token;
try {
  const page = await fetch(base);
  assert.equal(page.status, 200);
  assert.match(await page.text(), /Think in parallel/);
  assert.equal((await call({ email: "invalid" })).status, 400);
  assert.equal((await call({ email, platform: "unknown" })).status, 400);
  assert.equal((await call({ email, website: "spam.example" })).status, 400);
  assert.equal((await call({ email }, { headers: { Origin: "https://other.example" } })).status, 403);
  assert.equal((await call(null)).status, 400);
  const joined = await call({ email: email.toUpperCase(), platform: "linux" });
  assert.equal(joined.status, 200);
  token = (await joined.json()).removalToken;
  assert.equal(token.length, 72);
  const duplicate = await call({ email, platform: "macos" });
  assert.equal(duplicate.status, 200);
  assert.equal((await duplicate.json()).removalToken, undefined, "Duplicate email must not grant removal access.");
  assert.equal((await call({ token: "invalid" }, { method: "DELETE" })).status, 400);
  assert.equal((await call({ token }, { method: "DELETE" })).status, 200);
  token = undefined;
  const rejoined = await call({ email });
  token = (await rejoined.json()).removalToken;
  assert.equal(token.length, 72, "Deleted signup must be able to join again.");
  console.log("Passed: page response, validation, platform handling, bot trap, origin check, signup, deduplication, protected removal, and rejoining.");
} finally {
  if (token) assert.equal((await call({ token }, { method: "DELETE" })).status, 200);
}
