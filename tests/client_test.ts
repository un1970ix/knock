import { KnockError, revealEmail } from "../client/index.js";

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message);
}

Deno.test("The browser solver produces a valid SHA-256 proof", async () => {
  const worker = new Worker(new URL("../client/solver.js", import.meta.url), {
    type: "module",
  });
  try {
    const result = await new Promise<{ nonce: string }>((resolve, reject) => {
      worker.onmessage = (event) => resolve(event.data);
      worker.onerror = reject;
      worker.postMessage({
        token: "knock-test-v1",
        difficulty: 16,
        expires_at: Math.floor(Date.now() / 1000) + 120,
      });
    });
    assert(/^\d+$/.test(result.nonce), "Solver returned no nonce.");
    const hash = new Uint8Array(
      await crypto.subtle.digest(
        "SHA-256",
        new TextEncoder().encode(`knock-test-v1.${result.nonce}`),
      ),
    );
    assert(
      hash[0] === 0 && hash[1] === 0,
      "Proof does not satisfy the difficulty.",
    );
  } finally {
    worker.terminate();
  }
});

Deno.test("The solver rejects excessive difficulty instead of consuming CPU", async () => {
  const worker = new Worker(new URL("../client/solver.js", import.meta.url), {
    type: "module",
  });
  try {
    const result = await new Promise<{ error: string }>((resolve, reject) => {
      worker.onmessage = (event) => resolve(event.data);
      worker.onerror = reject;
      worker.postMessage({
        token: "knock-test-v1",
        difficulty: 255,
        expires_at: 2_000_000_000,
      });
    });
    assert(
      result.error === "invalid_challenge",
      "Unsafe challenge was accepted.",
    );
  } finally {
    worker.terminate();
  }
});

Deno.test("The client rejects insecure or ambiguous endpoints without a request", async () => {
  for (
    const endpoint of [
      "http://example.com",
      "https://user:pass@example.com",
      "https://example.com/api",
      "https://example.com?x=1",
    ]
  ) {
    let failure: unknown;
    try {
      await revealEmail({ endpoint, site: "example", signal: undefined });
    } catch (error) {
      failure = error;
    }
    assert(
      failure instanceof KnockError && failure.code === "invalid_configuration",
      "Invalid endpoint was accepted.",
    );
  }
});
