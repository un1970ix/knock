self.onmessage = async ({ data }) => {
  try {
    const { token, difficulty, expires_at: expires } = data;
    if (
      typeof token !== "string" || token.length > 2048 ||
      !Number.isInteger(difficulty) || difficulty < 16 || difficulty > 20 ||
      !Number.isSafeInteger(expires)
    ) throw new Error("invalid_challenge");
    const encoder = new TextEncoder();
    const started = performance.now();
    for (let nonce = 0; nonce <= 0xffffffff; nonce++) {
      if (nonce % 256 === 0) {
        if (Date.now() >= expires * 1000) throw new Error("challenge_expired");
        if (performance.now() - started >= 60_000) {
          throw new Error("solver_timeout");
        }
      }
      const hash = new Uint8Array(
        await crypto.subtle.digest(
          "SHA-256",
          encoder.encode(`${token}.${nonce}`),
        ),
      );
      let zeros = 0;
      for (const byte of hash) {
        zeros += Math.clz32(byte) - 24;
        if (byte !== 0) break;
      }
      if (zeros >= difficulty) {
        self.postMessage({ nonce: String(nonce) });
        self.close();
        return;
      }
    }
    throw new Error("solver_exhausted");
  } catch (error) {
    self.postMessage({ error: error.message ?? "solver_failed" });
    self.close();
  }
};
