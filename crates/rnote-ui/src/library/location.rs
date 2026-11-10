//! Where the library lives.

use std::path::{Path, PathBuf};

/// The library's folder: the `library-dir` setting when set, else `Rnote` in the
/// documents folder (`documents`, from XDG), else in `home/Documents`.
pub(crate) fn library_root(setting: &str, documents: Option<PathBuf>, home: &Path) -> PathBuf {
    if !setting.is_empty() {
        return PathBuf::from(setting);
    }
    documents
        .unwrap_or_else(|| home.join("Documents"))
        .join("Rnote")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::library_root;

    #[test]
    fn library_root_uses_the_setting_when_set() {
        let root = library_root(
            "/mnt/Storage/HandNotes",
            Some(PathBuf::from("/d")),
            Path::new("/h"),
        );

        assert_eq!(root, PathBuf::from("/mnt/Storage/HandNotes"));
    }

    #[test]
    fn library_root_defaults_to_documents_rnote() {
        let root = library_root(
            "",
            Some(PathBuf::from("/home/u/Documentos")),
            Path::new("/home/u"),
        );

        assert_eq!(root, PathBuf::from("/home/u/Documentos/Rnote"));
    }

    #[test]
    fn library_root_falls_back_to_home_documents() {
        assert_eq!(
            library_root("", None, Path::new("/home/u")),
            PathBuf::from("/home/u/Documents/Rnote")
        );
    }
}
