import { existsSync } from "node:fs";
import { delimiter, join, resolve } from "node:path";
import { spawn } from "node:child_process";

const localBin = resolve("node_modules/.bin");
const candidates = (process.env.PATH ?? "")
  .split(delimiter)
  .filter((directory) => resolve(directory) !== localBin)
  .map((directory) => join(directory, "electron"));
const executable = process.env.ELECTRON_BINARY || candidates.find(existsSync);

if (!executable) {
  console.error("Electron runtime not found. Run this command inside `nix develop`.");
  process.exit(1);
}

const child = spawn(executable, process.argv.slice(2), {
  env: process.env,
  stdio: "inherit",
});
for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => child.kill(signal));
}
child.on("exit", (code, signal) => {
  if (signal) process.kill(process.pid, signal);
  else process.exit(code ?? 1);
});
