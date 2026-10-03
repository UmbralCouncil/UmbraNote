use umbra_note::{editor::MarkdownAction, markdown, storage::Repository};

#[test]
fn load_edit_format_save_reload_preserves_markdown_structure() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Research.md");
    let original = "# Research\n\nA portable note.\n\n- [ ] Verify capture\n\n| Key | Value |\n| --- | --- |\n| mode | safe |\n\n![diagram](Research/diagram.png)\n";
    std::fs::write(&path, original).unwrap();

    let repo = Repository::new(temp.path()).unwrap();
    let mut document = repo.load(&path).unwrap();
    let start = document.body().find("portable").unwrap();
    let char_start = document.body()[..start].chars().count();
    document
        .format(
            char_start..char_start + "portable".chars().count(),
            MarkdownAction::Bold,
        )
        .unwrap();
    repo.save(&mut document).unwrap();

    let reloaded = repo.load(&path).unwrap();
    assert_eq!(
        reloaded.body(),
        original.replacen("portable", "**portable**", 1)
    );
    let structure = markdown::analyze(&reloaded.body());
    assert_eq!(structure.headings, ["Research"]);
    assert!(reloaded.body().contains("- [ ] Verify capture"));
    assert!(reloaded.body().contains("| Key | Value |"));
    assert!(reloaded.body().contains("![diagram](Research/diagram.png)"));
}
