use umbra_note::{app::AppState, editor::MarkdownAction, search};

#[test]
fn complete_restart_and_edit_workflow_uses_markdown_files() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_path_buf();

    let mut first_session = AppState::load(root.clone(), None).unwrap();
    first_session.create().unwrap();
    let document = first_session.active.as_mut().unwrap();
    document.set_body("# Field Notes\n\nhello world\n");
    let world = document.body().find("world").unwrap();
    let char_start = document.body()[..world].chars().count();
    document
        .format(char_start..char_start + 5, MarkdownAction::Bold)
        .unwrap();
    first_session.rename("Field Notes".into()).unwrap();
    first_session.save().unwrap();
    let path = first_session.active.as_ref().unwrap().path.clone();
    drop(first_session);

    let mut second_session = AppState::load(root, None).unwrap();
    assert_eq!(search::titles(&second_session.notes, "field").len(), 1);
    second_session.open(path.clone()).unwrap();
    assert_eq!(
        second_session.active.as_ref().unwrap().body(),
        "# Field Notes\n\nhello **world**\n"
    );
    second_session
        .active
        .as_mut()
        .unwrap()
        .set_body("# Field Notes\n\nhello **world** again 🟣\n");
    second_session.save().unwrap();

    let final_session = AppState::load(temp.path().to_path_buf(), Some(path)).unwrap();
    let final_document = final_session.active.unwrap();
    assert_eq!(final_document.title, "Field Notes");
    assert_eq!(
        final_document.body(),
        "# Field Notes\n\nhello **world** again 🟣\n"
    );
}
