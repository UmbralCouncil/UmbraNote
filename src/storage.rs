use crate::editor::Document;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NOTE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSummary {
    pub path: PathBuf,
    pub title: String,
    pub modified: SystemTime,
}

impl NoteSummary {
    pub fn new(path: PathBuf, title: impl Into<String>) -> Self {
        Self {
            path,
            title: title.into(),
            modified: SystemTime::UNIX_EPOCH,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteMetadata {
    /// Stable application identity; the Markdown filename remains user-controlled.
    #[serde(default = "new_note_id")]
    pub id: String,
    #[serde(default = "unix_millis")]
    pub created_unix_ms: u64,
    #[serde(default = "unix_millis")]
    pub modified_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub last_opened_unix: u64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collaboration_id: Option<String>,
}

impl NoteMetadata {
    fn new() -> Self {
        let now = unix_millis();
        Self {
            id: new_note_id(),
            created_unix_ms: now,
            modified_unix_ms: now,
            title: None,
            pinned: false,
            last_opened_unix: 0,
            tags: Vec::new(),
            collaboration_id: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Repository {
    root: PathBuf,
}

impl Repository {
    pub fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn discover(&self) -> io::Result<Vec<NoteSummary>> {
        let mut notes = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("md") {
                continue;
            }
            let modified = entry
                .metadata()?
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let title = self
                .metadata(&path)
                .ok()
                .and_then(|metadata| metadata.title)
                .or_else(|| {
                    fs::read_to_string(&path)
                        .ok()
                        .and_then(|body| first_heading(&body))
                })
                .unwrap_or_else(|| display_stem(&path));
            notes.push(NoteSummary {
                path,
                title,
                modified,
            });
        }
        notes.sort_by(|a, b| {
            b.modified
                .cmp(&a.modified)
                .then_with(|| a.title.cmp(&b.title))
        });
        Ok(notes)
    }

    pub fn load(&self, path: impl AsRef<Path>) -> io::Result<Document> {
        let path = path.as_ref();
        let body = fs::read_to_string(path)?;
        let title = self
            .metadata(path)
            .ok()
            .and_then(|metadata| metadata.title)
            .or_else(|| first_heading(&body))
            .unwrap_or_else(|| display_stem(path));
        Ok(Document::new(path.to_path_buf(), title, body))
    }

    pub fn metadata(&self, note_path: impl AsRef<Path>) -> io::Result<NoteMetadata> {
        let sidecar = metadata_path(note_path.as_ref());
        match fs::read(&sidecar) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(NoteMetadata::new()),
            Err(error) => Err(error),
        }
    }

    /// Conventional sibling directory for binaries referenced by ordinary
    /// Markdown links, e.g. `Research/diagram.png` from `Research.md`.
    pub fn attachment_dir(&self, note_path: impl AsRef<Path>) -> PathBuf {
        let note_path = note_path.as_ref();
        note_path.with_extension("")
    }

    pub fn create(&self, desired_title: &str) -> io::Result<Document> {
        let title = clean_title(desired_title);
        let path = unique_path(&self.root, &slug(&title));
        let mut doc = Document::new(path, title.clone(), format!("# {title}\n\n"));
        self.save(&mut doc)?;
        Ok(doc)
    }

    /// Writes and fsyncs a sibling temporary file before replacing the note.
    pub fn save(&self, doc: &mut Document) -> io::Result<()> {
        atomic_write(&doc.path, doc.body().as_bytes())?;
        let mut metadata = self.metadata(&doc.path)?;
        metadata.modified_unix_ms = unix_millis();
        metadata.title = Some(doc.title.clone());
        write_json_atomic(&metadata_path(&doc.path), &metadata)?;
        doc.mark_saved();
        Ok(())
    }

    pub fn rename(&self, doc: &mut Document, title: &str) -> io::Result<()> {
        let title = clean_title(title);
        let new_path = unique_path_except(&self.root, &slug(&title), &doc.path);
        if new_path != doc.path {
            let old_metadata = metadata_path(&doc.path);
            let new_metadata = metadata_path(&new_path);
            let old_attachments = self.attachment_dir(&doc.path);
            let new_attachments = self.attachment_dir(&new_path);
            fs::rename(&doc.path, &new_path)?;
            if old_metadata.exists()
                && let Err(error) = fs::rename(&old_metadata, &new_metadata)
            {
                let _ = fs::rename(&new_path, &doc.path);
                return Err(error);
            }
            if old_attachments.is_dir()
                && let Err(error) = fs::rename(&old_attachments, &new_attachments)
            {
                if new_metadata.exists() {
                    let _ = fs::rename(&new_metadata, &old_metadata);
                }
                let _ = fs::rename(&new_path, &doc.path);
                return Err(error);
            }
        }
        doc.path = new_path;
        doc.rename(title);
        let mut metadata = self.metadata(&doc.path)?;
        metadata.title = Some(doc.title.clone());
        metadata.modified_unix_ms = unix_millis();
        write_json_atomic(&metadata_path(&doc.path), &metadata)?;
        Ok(())
    }
}

fn metadata_path(note_path: &Path) -> PathBuf {
    note_path.with_extension("md.umbra.json")
}

fn write_json_atomic(path: &Path, metadata: &NoteMetadata) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut terminated = bytes;
    terminated.push(b'\n');
    atomic_write(path, &terminated)
}

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "file has no parent directory")
    })?;
    fs::create_dir_all(parent)?;
    let sequence = NOTE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("note");
    let tmp = parent.join(format!(
        ".{file_name}.umbra-tmp-{}-{sequence}",
        std::process::id()
    ));

    let result = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        if let Ok(existing) = fs::metadata(path) {
            file.set_permissions(existing.permissions())?;
        }
        file.write_all(contents)?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn new_note_id() -> String {
    let sequence = NOTE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}-{sequence:x}", unix_millis(), std::process::id())
}

fn first_heading(body: &str) -> Option<String> {
    body.lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
}

fn display_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Untitled")
        .replace(['-', '_'], " ")
}

fn clean_title(title: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        "Untitled note".into()
    } else {
        title.into()
    }
}

fn slug(title: &str) -> String {
    let value = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>();
    let value = value
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if value.is_empty() {
        "untitled-note".into()
    } else {
        value
    }
}

fn unique_path(root: &Path, stem: &str) -> PathBuf {
    unique_path_except(root, stem, Path::new(""))
}

fn unique_path_except(root: &Path, stem: &str, except: &Path) -> PathBuf {
    for suffix in 0.. {
        let name = if suffix == 0 {
            format!("{stem}.md")
        } else {
            format!("{stem}-{suffix}.md")
        };
        let candidate = root.join(name);
        let sidecar = metadata_path(&candidate);
        let attachments = candidate.with_extension("");
        if candidate == except
            || (!candidate.exists() && !sidecar.exists() && !attachments.exists())
        {
            return candidate;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_create_save_discover_and_rename() {
        let temp = tempfile::tempdir().unwrap();
        let repo = Repository::new(temp.path()).unwrap();
        let mut doc = repo.create("Purple Ideas").unwrap();
        doc.set_body("# Purple Ideas\n\nA durable thought.");
        repo.save(&mut doc).unwrap();
        assert!(!doc.is_dirty());
        assert_eq!(repo.discover().unwrap()[0].title, "Purple Ideas");
        repo.rename(&mut doc, "Night Notes").unwrap();
        assert!(doc.path.ends_with("night-notes.md"));
        let reloaded = repo.load(&doc.path).unwrap();
        assert_eq!(reloaded.title, "Night Notes");
        assert_eq!(reloaded.body(), "# Purple Ideas\n\nA durable thought.");
    }

    #[test]
    fn duplicate_names_never_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let repo = Repository::new(temp.path()).unwrap();
        let first = repo.create("Same").unwrap();
        let second = repo.create("Same").unwrap();
        assert_ne!(first.path, second.path);
    }

    #[test]
    fn metadata_is_sidecar_and_id_survives_saves_and_rename() {
        let temp = tempfile::tempdir().unwrap();
        let repo = Repository::new(temp.path()).unwrap();
        let mut doc = repo.create("Research").unwrap();
        let original = repo.metadata(&doc.path).unwrap();
        assert!(!doc.body().contains(&original.id));
        doc.set_body("# Research\n\nPortable Markdown.");
        repo.save(&mut doc).unwrap();
        assert_eq!(repo.metadata(&doc.path).unwrap().id, original.id);
        repo.rename(&mut doc, "Field Research").unwrap();
        assert_eq!(repo.metadata(&doc.path).unwrap().id, original.id);
    }

    #[test]
    fn attachment_directory_is_a_normal_markdown_relative_path() {
        let temp = tempfile::tempdir().unwrap();
        let repo = Repository::new(temp.path()).unwrap();
        let path = temp.path().join("Research.md");
        assert_eq!(repo.attachment_dir(path), temp.path().join("Research"));
    }

    #[test]
    fn older_sidecars_gain_defaults_without_touching_markdown() {
        let temp = tempfile::tempdir().unwrap();
        let repo = Repository::new(temp.path()).unwrap();
        let note = temp.path().join("Legacy.md");
        fs::write(&note, "# Legacy\n").unwrap();
        fs::write(
            metadata_path(&note),
            r#"{"pinned":true,"last_opened_unix":7}"#,
        )
        .unwrap();
        let metadata = repo.metadata(&note).unwrap();
        assert!(metadata.pinned);
        assert!(!metadata.id.is_empty());
        assert_eq!(fs::read_to_string(note).unwrap(), "# Legacy\n");
    }

    #[test]
    fn empty_unicode_and_large_documents_round_trip_exactly() {
        let temp = tempfile::tempdir().unwrap();
        let repo = Repository::new(temp.path()).unwrap();
        for (name, body) in [
            ("Empty", String::new()),
            (
                "Unicode",
                "# 日本語 🟣\n\nλ, café, مرحبا, \0 replacement-free".into(),
            ),
            (
                "Large",
                format!("# Large\n\n{}", "technical prose 🟣\n".repeat(100_000)),
            ),
        ] {
            let mut doc = repo.create(name).unwrap();
            doc.set_body(&body);
            repo.save(&mut doc).unwrap();
            assert_eq!(repo.load(&doc.path).unwrap().body(), body);
        }
    }

    #[cfg(unix)]
    #[test]
    fn failed_atomic_write_preserves_previous_file() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let repo = Repository::new(temp.path()).unwrap();
        let mut doc = repo.create("Protected").unwrap();
        let original = doc.body();
        doc.set_body("replacement that must not land");

        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let result = repo.save(&mut doc);
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o755)).unwrap();

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&doc.path).unwrap(), original);
        assert!(doc.is_dirty());
    }

    #[test]
    fn interrupted_temp_artifacts_are_not_discovered_or_reused() {
        let temp = tempfile::tempdir().unwrap();
        let repo = Repository::new(temp.path()).unwrap();
        let stale = temp.path().join(".note.md.umbra-tmp-interrupted");
        fs::write(&stale, "partial").unwrap();
        let mut doc = repo.create("Note").unwrap();
        doc.set_body("# Note\n\ncomplete");
        repo.save(&mut doc).unwrap();
        assert_eq!(repo.discover().unwrap().len(), 1);
        assert_eq!(fs::read_to_string(doc.path).unwrap(), "# Note\n\ncomplete");
    }
}
