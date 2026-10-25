const originalHash =
  "85a0fd68160392c7b9233a2a3406a94d61ab2a84d6dc192ebcb758774a76ee61";
const patchedHash =
  "a5e222ee49c88c29847c265faa8a83d76d718a80401e74feab61557629650203";

async function hash(source: string) {
  const bytes = new Uint8Array(
    await crypto.subtle.digest("SHA-256", new TextEncoder().encode(source)),
  );
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join(
    "",
  );
}

export async function prepareWrangler(install = false) {
  const path = "node_modules/wrangler/wrangler-dist/cli.js";
  const source = await Deno.readTextFile(path);
  const checksum = await hash(source);
  if (checksum === patchedHash) return;
  if (!install || checksum !== originalHash) {
    throw new Error(
      "Wrangler needs its verified Undici patch. Run just setup.",
    );
  }
  const prefix = "// ../../node_modules/.pnpm/undici@7.29.1/";
  const start = source.indexOf(prefix);
  const entry = source.indexOf(`${prefix}node_modules/undici/index.js`);
  const end = source.indexOf("\n// ", entry + 4);
  const patched = source.slice(0, start) +
    'var require_undici = () => require("undici");\n' + source.slice(end);
  if (await hash(patched) !== patchedHash) {
    throw new Error("Unexpected Wrangler Undici patch result.");
  }
  await Deno.writeTextFile(path, patched);
}

if (import.meta.main) {
  if (Deno.args.length !== 1 || Deno.args[0] !== "install") {
    throw new Error("Choose install.");
  }
  await prepareWrangler(true);
}
