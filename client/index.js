export class KnockError extends Error {
  constructor(code, status = 0) {
    super(code);
    this.name = "KnockError";
    this.code = code;
    this.status = status;
  }
}

export async function revealEmail({ endpoint, site, signal }) {
  const base = new URL(endpoint);
  const local = ["localhost", "127.0.0.1", "[::1]"].includes(base.hostname);
  if (
    (base.protocol !== "https:" && !(local && base.protocol === "http:")) ||
    base.username || base.password || base.search || base.hash ||
    base.pathname !== "/" || !/^[a-z0-9-]{1,64}$/.test(site)
  ) throw new KnockError("invalid_configuration");

  const deadline = AbortSignal.timeout(90_000);
  const cancellation = signal ? AbortSignal.any([signal, deadline]) : deadline;

  async function post(path, body) {
    const response = await fetch(new URL(path, base), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
      credentials: "omit",
      cache: "no-store",
      redirect: "error",
      referrerPolicy: "no-referrer",
      signal: cancellation,
    });
    const result = await response.json();
    if (!response.ok) {
      throw new KnockError(result.error ?? "request_failed", response.status);
    }
    return result;
  }

  const challenge = await post("/v1/challenge", { site });
  if (
    challenge.version !== 1 || challenge.algorithm !== "sha256" ||
    typeof challenge.token !== "string" || challenge.token.length > 2048 ||
    !Number.isInteger(challenge.difficulty) || challenge.difficulty < 16 ||
    challenge.difficulty > 20 || !Number.isSafeInteger(challenge.expires_at)
  ) throw new KnockError("invalid_challenge");

  const nonce = await solve(challenge, cancellation);
  const result = await post("/v1/reveal", {
    site,
    token: challenge.token,
    nonce,
  });
  if (
    typeof result.email !== "string" || result.email.length > 254 ||
    !/^[a-z0-9.!#$%&'*+\-/=?^_`{|}~]+@[a-z0-9.-]+$/i.test(result.email)
  ) throw new KnockError("invalid_response");
  return result.email;
}

function solve(challenge, signal) {
  signal.throwIfAborted();
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL("./solver.js", import.meta.url), {
      type: "module",
      name: "knock",
    });
    const stop = () => {
      worker.terminate();
      signal.removeEventListener("abort", abort);
    };
    const abort = () => {
      stop();
      reject(signal.reason);
    };
    signal.addEventListener("abort", abort, { once: true });
    worker.onmessage = ({ data }) => {
      stop();
      if (typeof data.nonce === "string") resolve(data.nonce);
      else reject(new KnockError(data.error ?? "solver_failed"));
    };
    worker.onerror = () => {
      stop();
      reject(new KnockError("solver_failed"));
    };
    worker.postMessage(challenge);
  });
}
