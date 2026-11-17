import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";

const root = resolve(import.meta.dirname, "../../..");
// Defaults are the tracking journey's rig; the training journey passes its own.
const rig = process.env.BANSAYO_E2E_CONFIG ?? "config/sim-full-body.yaml";
const enginePort = process.env.BANSAYO_E2E_ENGINE_PORT ?? "17700";
const uiPort = process.env.BANSAYO_E2E_UI_PORT ?? "15173";
const directory = await mkdtemp(join(tmpdir(), "bansayo-e2e-"));
const config = (await readFile(join(root, rig), "utf8"))
  .replace("127.0.0.1:7700", `127.0.0.1:${enginePort}`)
  .replace(
    /data_dir: .*/,
    `data_dir: ${JSON.stringify(join(directory, "data"))}`,
  )
  .replace(/dirs: \[\.\/content\]/, `dirs: [${JSON.stringify(join(root, "content"))}]`)
  .replace(/http:\/\/localhost:5173/g, `http://localhost:${uiPort}`)
  .replace(/http:\/\/127.0.0.1:5173/g, `http://127.0.0.1:${uiPort}`);
// \r? because a Windows checkout has CRLF line endings.
const withOrigins = config.replace(
  /^api:\r?\n/m,
  `api:\n  allowed_origins: ["http://127.0.0.1:${uiPort}", "http://localhost:${uiPort}"]\n`,
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
