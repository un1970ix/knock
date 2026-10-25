import { createRequire } from "node:module";
import { homedir } from "node:os";
import { prepareWrangler } from "./patch-wrangler.ts";

const mode = Deno.args[0];
if (Deno.args.length !== 1 || !["check", "dev", "publish"].includes(mode)) {
  throw new Error("Choose check, dev, or publish.");
}
const root = Deno.cwd();
await prepareWrangler();
const directory = `${root}/.cache/wrangler`;
const require = createRequire(
  await Deno.realPath("node_modules/wrangler/package.json"),
);
const platform = Deno.build.os === "windows" ? "win32" : Deno.build.os;
const architecture = Deno.build.arch === "aarch64" ? "arm64" : "x64";
const esbuild = createRequire(require.resolve("esbuild")).resolve(
  `@esbuild/${platform}-${architecture}/bin/esbuild`,
);
const workerd = createRequire(require.resolve("workerd")).resolve(
  `@cloudflare/workerd-${platform === "darwin" ? "darwin" : "linux"}-${
    architecture === "arm64" ? "arm64" : "64"
  }/bin/workerd`,
);
await Deno.mkdir(`${directory}/tmp`, { recursive: true });
const environment: Record<string, string> = {
  PATH: Deno.env.get("PATH") ?? "/usr/bin:/bin",
  DENO_DIR: `${root}/.cache/deno`,
  XDG_CONFIG_HOME: directory,
  XDG_CACHE_HOME: directory,
  TMPDIR: `${directory}/tmp`,
  WRANGLER_LOG_PATH: `${directory}/logs`,
  WRANGLER_SEND_METRICS: "false",
  WRANGLER_SEND_ERROR_REPORTS: "false",
  CLOUDFLARE_LOAD_DEV_VARS_FROM_DOT_ENV: "false",
  CLOUDFLARE_CF_FETCH_ENABLED: "false",
  CI: "true",
  NO_COLOR: "1",
  DO_NOT_TRACK: "1",
  ESBUILD_BINARY_PATH: esbuild,
  MINIFLARE_WORKERD_PATH: workerd,
};
if (mode === "publish") {
  if (!Deno.env.get("CLOUDFLARE_API_TOKEN")) {
    throw new Error("Cloudflare build authentication is unavailable.");
  }
  for (
    const name of [
      "CLOUDFLARE_ACCOUNT_ID",
      "CLOUDFLARE_API_TOKEN",
      "WRANGLER_CI_MATCH_TAG",
    ]
  ) {
    const value = Deno.env.get(name);
    if (value) environment[name] = value;
  }
}
const command = new Deno.Command("deno", {
  args: [
    "run",
    "--frozen",
    "--cached-only",
    "--no-prompt",
    `--allow-read=.,${homedir()}/.wrangler`,
    `--deny-read=.git,.env,.env.local,.env.production,${homedir()}/.wrangler/config`,
    "--allow-write=.cache,.wrangler",
    "--allow-env",
    `--allow-run=${esbuild},${workerd}`,
    "--allow-sys=homedir,hostname,cpus,osRelease,networkInterfaces,uid,gid",
    ...(mode === "publish"
      ? ["--allow-net=api.cloudflare.com"]
      : mode === "dev"
      ? ["--allow-net=127.0.0.1,localhost"]
      : []),
    "node_modules/wrangler/wrangler-dist/cli.js",
    ...(mode === "dev"
      ? [
        "dev",
        "--local",
        "--ip",
        "127.0.0.1",
        "--port",
        "8787",
        "--log-level",
        "warn",
        "--persist-to",
        ".cache/state",
      ]
      : ["deploy", ...(mode === "check" ? ["--dry-run"] : [])]),
    "--config",
    `${root}/wrangler.jsonc`,
    "--no-bundle",
  ],
  clearEnv: true,
  env: environment,
  stdin: "null",
  stdout: "inherit",
  stderr: "inherit",
});
Deno.exit((await command.spawn().status).code);
