use crate::{
    editor::Document,
    storage::{NoteSummary, Repository},
};
use std::path::PathBuf;

#[derive(Clone)]
pub struct AppState {
    pub repository: Repository,
    pub notes: Vec<NoteSummary>,
    pub active: Option<Document>,
    pub generation: u64,
}

impl AppState {
    pub fn load(root: PathBuf, initial: Option<PathBuf>) -> anyhow::Result<Self> {
        let repository = Repository::new(root)?;
        let notes = repository.discover()?;
        let active = initial
            .as_ref()
            .map(|path| repository.load(path))
            .transpose()?;
        Ok(Self {
            repository,
            notes,
            active,
            generation: 0,
        })
    }

    pub fn refresh(&mut self) -> anyhow::Result<()> {
        self.notes = self.repository.discover()?;
        Ok(())
    }

    pub fn create(&mut self) -> anyhow::Result<()> {
        self.save()?;
        self.active = Some(self.repository.create("Untitled note")?);
        self.refresh()?;
        self.generation += 1;
        Ok(())
    }

    pub fn open(&mut self, path: PathBuf) -> anyhow::Result<()> {
        if self.active.as_ref().is_some_and(|doc| doc.path == path) {
            return Ok(());
        }
        self.save()?;
        self.active = Some(self.repository.load(path)?);
        self.generation += 1;
        Ok(())
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        if let Some(doc) = &mut self.active {
            self.repository.save(doc)?;
        }
        self.refresh()?;
        Ok(())
    }

    pub fn rename(&mut self, title: String) -> anyhow::Result<()> {
        if let Some(doc) = &mut self.active {
            self.repository.rename(doc, &title)?;
        }
        self.refresh()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_command_persists_and_selects_markdown_note() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = AppState::load(temp.path().to_path_buf(), None).unwrap();
        app.create().unwrap();

        let active = app.active.as_ref().expect("new note must become active");
        assert_eq!(
            active.path.extension().and_then(|value| value.to_str()),
            Some("md")
        );
        assert!(active.path.exists());
        assert_eq!(
            std::fs::read_to_string(&active.path).unwrap(),
            "# Untitled note\n\n"
        );
        assert_eq!(app.notes.len(), 1);
    }
}
