use crate::{Notebook, NotebookKind, Section, SortOrder, counts, filter, sort};

fn notebook(path: &str, modified_ms: i64, tags: &[&str]) -> Notebook {
    let (folder, file_name) = match path.split_once('/') {
        Some((folder, file_name)) => (Some(folder.to_owned()), file_name),
        None => (None, path),
    };
    Notebook {
        path: path.to_owned(),
        name: file_name.rsplit_once('.').map_or(file_name, |(stem, _)| stem).to_owned(),
        folder,
        kind: NotebookKind::Rnote,
        modified_ms,
        size: 1,
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
    }
}

fn sample() -> Vec<Notebook> {
    vec![
        notebook("root.rnote", 3, &["t1"]),
        notebook("Algebra/a.rnote", 1, &["t1", "t2"]),
        notebook("Algebra Lineal/b.rnote", 2, &[]),
    ]
}

fn paths(notebooks: &[Notebook]) -> Vec<&str> {
    notebooks.iter().map(|notebook| notebook.path.as_str()).collect()
}

#[test]
fn filter_all_returns_every_notebook() {
    assert_eq!(filter(&sample(), &Section::All), sample());
}

#[test]
fn filter_uncategorized_keeps_only_root_notebooks() {
    assert_eq!(paths(&filter(&sample(), &Section::Uncategorized)), ["root.rnote"]);
}

#[test]
fn filter_folder_matches_the_folder_name_exactly() {
    let filtered = filter(&sample(), &Section::Folder("Algebra".to_owned()));

    assert_eq!(paths(&filtered), ["Algebra/a.rnote"]);
}

#[test]
fn filter_tag_keeps_notebooks_with_that_tag() {
    let filtered = filter(&sample(), &Section::Tag("t1".to_owned()));

    assert_eq!(paths(&filtered), ["root.rnote", "Algebra/a.rnote"]);
}

#[test]
fn filter_trash_is_empty() {
    assert!(filter(&sample(), &Section::Trash).is_empty());
}

#[test]
fn sort_by_modified_puts_the_newest_first() {
    let mut notebooks = sample();

    sort(&mut notebooks, SortOrder::Modified);

    assert_eq!(paths(&notebooks), ["root.rnote", "Algebra Lineal/b.rnote", "Algebra/a.rnote"]);
}

#[test]
fn sort_by_name_ignores_case_and_breaks_ties_by_newest() {
    let mut notebooks = vec![
        notebook("b.rnote", 9, &[]),
        notebook("A.rnote", 1, &[]),
        notebook("F/a.rnote", 5, &[]),
    ];

    sort(&mut notebooks, SortOrder::Name);

    assert_eq!(paths(&notebooks), ["F/a.rnote", "A.rnote", "b.rnote"]);
}

#[test]
fn sort_order_defaults_to_modified() {
    assert_eq!(SortOrder::default(), SortOrder::Modified);
}

#[test]
fn counts_include_empty_folders() {
    let folders = ["Algebra".to_owned(), "Algebra Lineal".to_owned(), "Empty".to_owned()];

    let counts = counts(&sample(), &folders, 4);

    assert_eq!(counts.all, 3);
    assert_eq!(counts.uncategorized, 1);
    assert_eq!(counts.folders["Algebra"], 1);
    assert_eq!(counts.folders["Algebra Lineal"], 1);
    assert_eq!(counts.folders["Empty"], 0);
    assert_eq!(counts.trash, 4);
}

#[test]
fn counts_count_a_notebook_once_per_tag() {
    let counts = counts(&sample(), &[], 0);

    assert_eq!(counts.tags["t1"], 2);
    assert_eq!(counts.tags["t2"], 1);
    assert_eq!(counts.tags.len(), 2);
}
