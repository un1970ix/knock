const mode = Deno.args[0];
if (Deno.args.length !== 1 || !["install", "check"].includes(mode)) {
  throw new Error("Choose install or check.");
}

const tools = [
  {
    name: "worker-build",
    version: "0.8.7",
    checksum:
      "1051c05ef088889a30ad3bad40c22a716ba8ca8d02a3fb01cef8c143ba952222",
  },
  {
    name: "cargo-audit",
    version: "0.22.2",
    checksum:
      "700c2b240f7fd330c24b675fe429f73a5b676531fcc6300400b2b67f155ba12a",
  },
  {
    name: "just",
    version: "1.59.0",
    checksum:
      "7e46c1911c9c129f5b4f05edb3cd9aa75bba81e0198bbdbcf6181f7bcc21d74e",
  },
];

async function hash(bytes: Uint8Array<ArrayBuffer>) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join(
    "",
  );
}

async function run(command: string, args: string[]) {
  const status = await new Deno.Command(command, { args, stdin: "null" })
    .spawn()
    .status;
  if (!status.success) Deno.exit(status.code);
}

const directory = ".cache/tools";
const cargo = Deno.env.get("CARGO_HOME");
if (!cargo) throw new Error("Run through scripts/setup.sh.");
const installer = await Deno.readTextFile(import.meta.filename!);
const toolchain = await Deno.readTextFile("rust-toolchain.toml");
if (mode === "install") await Deno.mkdir(directory, { recursive: true });

for (const tool of tools) {
  const lock = await Deno.readTextFile(`tools/${tool.name}.lock`);
  const fingerprint = await hash(
    new TextEncoder().encode(installer + toolchain + lock),
  );
  const marker = `${directory}/${tool.name}.installed`;
  try {
    if (
      await Deno.readTextFile(marker) === fingerprint &&
      (await Deno.stat(`${cargo}/bin/${tool.name}`)).isFile
    ) continue;
  } catch (error) {
    if (!(error instanceof Deno.errors.NotFound)) throw error;
  }
  if (mode === "check") {
    throw new Error(`${tool.name} needs setup. Run just setup.`);
  }

  const archive = `${directory}/${tool.name}-${tool.version}.crate`;
  const response = await fetch(
    `https://static.crates.io/crates/${tool.name}/${tool.name}-${tool.version}.crate`,
    { redirect: "error" },
  );
  if (!response.ok) throw new Error(`Cannot download ${tool.name}.`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (await hash(bytes) !== tool.checksum) {
    throw new Error(`Checksum mismatch for ${tool.name}.`);
  }
  await Deno.writeFile(archive, bytes);
  const source = `${directory}/${tool.name}-${tool.version}`;
  try {
    await Deno.remove(source, { recursive: true });
  } catch (error) {
    if (!(error instanceof Deno.errors.NotFound)) throw error;
  }
  await run("tar", ["-xzf", archive, "-C", directory]);
  const manifest = `${source}/Cargo.toml`;
  if (tool.name === "worker-build") {
    const text = await Deno.readTextFile(manifest);
    const unused = '[dependencies.binary-install]\nversion = "0.4.1"\n\n';
    if (text.split(unused).length !== 2) {
      throw new Error("Unexpected worker-build manifest.");
    }
    await Deno.writeTextFile(manifest, text.replace(unused, ""));
  }
  await Deno.writeTextFile(`${source}/Cargo.lock`, lock);
  await run("cargo", ["fetch", "--locked", "--manifest-path", manifest]);
  await run("cargo", [
    "install",
    "--frozen",
    "--path",
    source,
    "--bin",
    tool.name,
    "--root",
    cargo,
    "--target-dir",
    `${directory}/target`,
    "--force",
  ]);
  await Deno.writeTextFile(marker, fingerprint);
}
