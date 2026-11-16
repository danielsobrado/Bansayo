import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";

const root = resolve(import.meta.dirname, "../../..");
const directory = await mkdtemp(join(tmpdir(), "bansayo-e2e-"));
const config = (await readFile(join(root, "config/sim-full-body.yaml"), "utf8"))
  .replace("127.0.0.1:7700", "127.0.0.1:17700")
  .replace(
    /data_dir: .*/,
    `data_dir: ${JSON.stringify(join(directory, "data"))}`,
  )
  .replace(/http:\/\/localhost:5173/g, "http://localhost:15173")
  .replace(/http:\/\/127.0.0.1:5173/g, "http://127.0.0.1:15173");
// \r? because a Windows checkout has CRLF line endings.
const withOrigins = config.replace(
  /^api:\r?\n/m,
  'api:\n  allowed_origins: [\"http://127.0.0.1:15173\", \"http://localhost:15173\"]\n',
);
const path = join(directory, "config.yaml");
await writeFile(path, withOrigins);
const engine = spawn(
  "cargo",
  ["run", "-p", "bansayo-engine", "--", "--config", path],
  { cwd: root, stdio: "inherit" },
);
for (const signal of ["SIGTERM", "SIGINT"])
  process.on(signal, () => engine.kill("SIGINT"));
engine.once("exit", async (code) => {
  await rm(directory, { recursive: true, force: true });
  process.exit(code ?? 0);
});
