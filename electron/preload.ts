import { contextBridge, ipcRenderer } from "electron";
import type { NoteFile, NoteSummary } from "../src-web/lib/types";

contextBridge.exposeInMainWorld("umbra", {
  listNotes: (): Promise<NoteSummary[]> => ipcRenderer.invoke("notes:list"),
  createNote: (): Promise<NoteFile> => ipcRenderer.invoke("notes:create"),
  importNote: (): Promise<NoteFile | undefined> => ipcRenderer.invoke("notes:import"),
  readNote: (path: string): Promise<NoteFile> => ipcRenderer.invoke("notes:read", path),
  saveNote: (path: string, content: string): Promise<NoteSummary> =>
    ipcRenderer.invoke("notes:save", path, content),
  renameNote: (path: string, title: string): Promise<NoteFile> =>
    ipcRenderer.invoke("notes:rename", path, title),
});
