export interface NoteSummary {
  path: string;
  title: string;
  modifiedMs: number;
}

export interface NoteFile extends NoteSummary {
  content: string;
}

export interface UmbraBridge {
  listNotes(): Promise<NoteSummary[]>;
  createNote(): Promise<NoteFile>;
  importNote(): Promise<NoteFile | undefined>;
  readNote(path: string): Promise<NoteFile>;
  saveNote(path: string, content: string): Promise<NoteSummary>;
  renameNote(path: string, title: string): Promise<NoteFile>;
}

declare global {
  interface Window {
    umbra: UmbraBridge;
  }
}
