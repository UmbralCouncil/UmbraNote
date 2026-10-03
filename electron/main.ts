import { app, BrowserWindow, dialog, ipcMain } from "electron";
import { promises as fs } from "node:fs";
import { homedir } from "node:os";
import { dirname, extname, join, parse, resolve, sep } from "node:path";
import type { NoteFile, NoteSummary } from "../src-web/lib/types";

const notesRoot = resolve(
  process.env.UMBRA_NOTE_DIR ?? join(homedir(), "local", "UmbraNote", "Notes"),
);

function safePath(candidate: string): string {
  const path = resolve(candidate);
  if (path !== notesRoot && !path.startsWith(`${notesRoot}${sep}`)) {
    throw new Error("The requested note is outside the notes directory.");
  }
  return path;
}

function titleFromPath(path: string): string {
  return parse(path).name.replaceAll(/[-_]+/g, " ");
}

function metadataPath(path: string): string {
  return `${path}.umbra.json`;
}

async function storedTitle(path: string): Promise<string> {
  try {
    const metadata = JSON.parse(await fs.readFile(metadataPath(path), "utf8")) as {
      title?: unknown;
    };
    if (typeof metadata.title === "string" && metadata.title.trim()) return metadata.title.trim();
  } catch {
    // Sidecars are optional. Ordinary Markdown files continue to work alone.
  }
  return titleFromPath(path);
}

async function writeTitle(path: string, title: string): Promise<void> {
  let metadata: Record<string, unknown> = {};
  try {
    metadata = JSON.parse(await fs.readFile(metadataPath(path), "utf8")) as Record<string, unknown>;
  } catch {
    // Create a new sidecar while preserving existing sidecar fields when present.
  }
  await atomicWrite(metadataPath(path), `${JSON.stringify({ ...metadata, title }, null, 2)}\n`);
}

async function summary(path: string): Promise<NoteSummary> {
  const [title, stat] = await Promise.all([storedTitle(path), fs.stat(path)]);
  return { path, title, modifiedMs: stat.mtimeMs };
}

async function listNotes(): Promise<NoteSummary[]> {
  await fs.mkdir(notesRoot, { recursive: true });
  const entries = await fs.readdir(notesRoot, { withFileTypes: true });
  const notes = await Promise.all(
    entries
      .filter((entry) => entry.isFile() && extname(entry.name).toLowerCase() === ".md")
      .map((entry) => summary(join(notesRoot, entry.name))),
  );
  return notes.sort((a, b) => b.modifiedMs - a.modifiedMs || a.title.localeCompare(b.title));
}

async function uniquePath(title: string, except?: string): Promise<string> {
  const slug =
    title
      .toLocaleLowerCase()
      .normalize("NFKD")
      .replace(/[^\p{Letter}\p{Number}]+/gu, "-")
      .replace(/^-|-$/g, "") || "untitled-note";
  for (let suffix = 0; ; suffix += 1) {
    const path = join(notesRoot, `${slug}${suffix ? `-${suffix}` : ""}.md`);
    if (path === except) return path;
    try {
      await fs.access(path);
      continue;
    } catch {
      // The Markdown path is available; also avoid orphaned sidecar collisions.
    }
    try {
      await fs.access(metadataPath(path));
    } catch {
      return path;
    }
  }
}

async function atomicWrite(path: string, content: string): Promise<void> {
  await fs.mkdir(dirname(path), { recursive: true });
  const temporary = join(dirname(path), `.${parse(path).base}.umbra-${process.pid}-${Date.now()}.tmp`);
  let handle: fs.FileHandle | undefined;
  try {
    handle = await fs.open(temporary, "wx", 0o600);
    await handle.writeFile(content, "utf8");
    await handle.sync();
    await handle.close();
    handle = undefined;
    await fs.rename(temporary, path);
    const directory = await fs.open(dirname(path), "r");
    await directory.sync();
    await directory.close();
  } catch (error) {
    await handle?.close().catch(() => undefined);
    await fs.unlink(temporary).catch(() => undefined);
    throw error;
  }
}

async function readNote(rawPath: string): Promise<NoteFile> {
  const path = safePath(rawPath);
  const content = await fs.readFile(path, "utf8");
  return { ...(await summary(path)), content };
}

ipcMain.handle("notes:list", listNotes);
ipcMain.handle("notes:create", async (): Promise<NoteFile> => {
  await fs.mkdir(notesRoot, { recursive: true });
  const path = await uniquePath("Untitled note");
  const content = "";
  await atomicWrite(path, content);
  await writeTitle(path, "Untitled note");
  return readNote(path);
});
ipcMain.handle("notes:import", async (): Promise<NoteFile | undefined> => {
  const result = await dialog.showOpenDialog({
    title: "Import Markdown note",
    properties: ["openFile"],
    filters: [{ name: "Markdown", extensions: ["md", "markdown", "mdown", "mkd"] }],
  });
  const sourcePath = result.filePaths[0];
  if (result.canceled || !sourcePath) return undefined;
  const content = await fs.readFile(sourcePath, "utf8");
  const title = titleFromPath(sourcePath);
  const destination = await uniquePath(title);
  await atomicWrite(destination, content);
  await writeTitle(destination, title);
  return readNote(destination);
});
ipcMain.handle("notes:read", (_event, path: string) => readNote(path));
ipcMain.handle("notes:save", async (_event, rawPath: string, content: string) => {
  const path = safePath(rawPath);
  await atomicWrite(path, content);
  return summary(path);
});
ipcMain.handle("notes:rename", async (_event, rawPath: string, rawTitle: string) => {
  const oldPath = safePath(rawPath);
  const title = rawTitle.trim() || "Untitled note";
  const newPath = await uniquePath(title, oldPath);
  const content = await fs.readFile(oldPath, "utf8");
  if (newPath !== oldPath) {
    await fs.rename(oldPath, newPath);
    await fs.rename(metadataPath(oldPath), metadataPath(newPath)).catch(() => undefined);
  }
  await writeTitle(newPath, title);
  return { ...(await summary(newPath)), content };
});

function createWindow(): void {
  const window = new BrowserWindow({
    width: 1220,
    height: 780,
    minWidth: 860,
    minHeight: 560,
    backgroundColor: "#070707",
    title: "Umbra Note",
    icon: join(__dirname, "../assets/note.png"),
    webPreferences: {
      preload: join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
  window.setMenuBarVisibility(false);
  const developmentUrl = process.env.VITE_DEV_SERVER_URL;
  if (developmentUrl) void window.loadURL(developmentUrl);
  else void window.loadFile(join(__dirname, "../dist/index.html"));
}

app.whenReady().then(() => {
  createWindow();
  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});
app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
