use crate::storage::NoteSummary;

pub fn titles<'a>(notes: &'a [NoteSummary], query: &str) -> Vec<&'a NoteSummary> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return notes.iter().collect();
    }
    let mut matches: Vec<_> = notes
        .iter()
        .filter(|note| note.title.to_lowercase().contains(&query))
        .collect();
    matches.sort_by_key(|note| {
        let title = note.title.to_lowercase();
        (title.find(&query).unwrap_or(usize::MAX), title)
    });
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn filters_case_insensitively() {
        let notes = vec![
            NoteSummary::new(PathBuf::from("one.md"), "Research"),
            NoteSummary::new(PathBuf::from("two.md"), "Daily log"),
        ];
        assert_eq!(titles(&notes, "SEARCH")[0].title, "Research");
    }
}
