const endpoint = "http://127.0.0.1:8787";
const origin = "http://127.0.0.1:8000";

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message);
}

async function request(
  path: string,
  body: unknown,
  expected: number,
  headers: Record<string, string> = {},
) {
  const response = await fetch(`${endpoint}${path}`, {
    method: "POST",
    headers: { Origin: origin, "Content-Type": "application/json", ...headers },
    body: JSON.stringify(body),
  });
  const result = await response.json();
  assert(
    response.status === expected,
    `${path} returned ${response.status}, expected ${expected}: ${
      JSON.stringify(result)
    }`,
  );
  assert(
    response.headers.get("Cache-Control") === "no-store",
    "Response could be cached.",
  );
  assert(!response.headers.has("Set-Cookie"), "Response sets a cookie.");
  return result;
}

await request("/unknown", {}, 404);
await request("/v1/challenge?x=1", {}, 404);
await request("/v1/challenge", { site: "example" }, 403, {
  Origin: "https://untrusted.example.com",
});
await request("/v1/challenge", { site: "example" }, 403, { Origin: "null" });
await request("/v1/challenge", { site: "unknown" }, 403);
await request("/v1/challenge", { site: "other" }, 403);
await request("/v1/challenge", { site: "example" }, 415, {
  "Content-Type": "text/plain",
});
await request(
  "/v1/challenge",
  { site: "example", padding: "x".repeat(4096) },
  413,
);
await request("/v1/challenge", { site: "example", extra: true }, 400);

const preflight = await fetch(`${endpoint}/v1/challenge`, {
  method: "OPTIONS",
  headers: {
    Origin: origin,
    "Access-Control-Request-Method": "POST",
    "Access-Control-Request-Headers": "content-type",
  },
});
assert(
  preflight.status === 204 &&
    preflight.headers.get("Access-Control-Allow-Origin") === origin,
  "Preflight failed.",
);
assert(
  !preflight.headers.has("Access-Control-Allow-Credentials"),
  "Credentialed CORS is enabled.",
);
await preflight.body?.cancel();

const challenge = await request("/v1/challenge", { site: "example" }, 200);
assert(
  !JSON.stringify(challenge).includes("hello@example.com"),
  "Challenge exposes the address.",
);
let nonce = 0;
const started = performance.now();
for (; nonce < 0xffffffff; nonce++) {
  const hash = new Uint8Array(
    await crypto.subtle.digest(
      "SHA-256",
      new TextEncoder().encode(`${challenge.token}.${nonce}`),
    ),
  );
  let zeros = 0;
  for (const byte of hash) {
    zeros += Math.clz32(byte) - 24;
    if (byte !== 0) break;
  }
  if (zeros >= challenge.difficulty) break;
  assert(performance.now() - started < 60_000, "Solving exceeded one minute.");
}
const body = { site: "example", token: challenge.token, nonce: String(nonce) };
await request("/v1/reveal", { ...body, token: `x${challenge.token}` }, 400);
const responses = await Promise.all(
  Array.from({ length: 8 }, () =>
    fetch(`${endpoint}/v1/reveal`, {
      method: "POST",
      headers: { Origin: origin, "Content-Type": "application/json" },
      body: JSON.stringify(body),
    })),
);
assert(
  responses.filter((response) => response.status === 200).length === 1,
  "A challenge was redeemed more than once.",
);
assert(
  responses.filter((response) => response.status === 409).length === 7,
  "Concurrent replays were not rejected.",
);
for (const response of responses) {
  const result = await response.json();
  assert(
    response.headers.get("Cache-Control") === "no-store",
    "Reveal could be cached.",
  );
  if (response.status === 200) {
    assert(result.email === "hello@example.com", "Incorrect email.");
  } else assert(!result.email, "Rejected replay exposes email.");
}

let limited = false;
for (let attempt = 0; attempt < 11; attempt++) {
  const response = await fetch(`${endpoint}/v1/challenge`, {
    method: "POST",
    headers: { Origin: origin, "Content-Type": "application/json" },
    body: JSON.stringify({ site: "example" }),
  });
  await response.body?.cancel();
  if (response.status === 429) {
    limited = true;
    break;
  }
  assert(response.status === 200, "Unexpected rate-limit response.");
}
assert(limited, "Issuance rate limit did not apply.");
console.log(
  "Runtime checks passed for CORS, request limits, proof verification, concurrent replay prevention, rate limiting, and uncached responses.",
);
