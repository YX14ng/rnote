//! What the sync needs from Google Drive, so it can run against the real one or a fake.

/// The name of the folder at the top of My Drive that mirrors the library.
pub const ROOT_FOLDER_NAME: &str = "Rnote";

/// A file in Drive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    /// Drive's id of the file.
    pub id: String,
    /// The id of its folder.
    pub parent: String,
    /// Its name, with the extension.
    pub name: String,
    /// The MD5 of its content, in lower-case hex (Drive's `md5Checksum`).
    pub md5: String,
    /// When it last changed (Drive's `modifiedTime`, RFC 3339).
    pub modified: String,
}

/// A folder in Drive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFolder {
    /// Drive's id of the folder.
    pub id: String,
    /// The id of its folder; `None` at the top of My Drive.
    pub parent: Option<String>,
    /// Its name.
    pub name: String,
}

/// Everything the app can see in Drive that isn't in Drive's trash.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    /// The files.
    pub files: Vec<RemoteFile>,
    /// The folders.
    pub folders: Vec<RemoteFolder>,
}

/// Why an operation on Drive failed.
#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    /// The access token was refused: sign in again.
    #[error("Google Drive refused the access token")]
    Unauthorized,
    /// Drive can't be reached.
    #[error("Google Drive can't be reached: {0}")]
    Offline(String),
    /// Anything else.
    #[error(transparent)]
    Failed(#[from] anyhow::Error),
}

/// The operations on Drive the sync uses. Nothing is ever deleted for good: removing a
/// file sends it to Drive's trash.
pub trait Remote {
    /// The folder named [`ROOT_FOLDER_NAME`] at the top of My Drive, if the app can see one.
    fn find_root(&mut self) -> Result<Option<String>, RemoteError>;

    /// Every file and folder the app can see that isn't trashed.
    fn list(&mut self) -> Result<Listing, RemoteError>;

    /// The content of the file `id`.
    fn download(&mut self, id: &str) -> Result<Vec<u8>, RemoteError>;

    /// Creates the file `name` in the folder `parent`.
    fn create_file(
        &mut self,
        parent: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<RemoteFile, RemoteError>;

    /// Replaces the content of the file `id`.
    fn update_file(&mut self, id: &str, bytes: &[u8]) -> Result<RemoteFile, RemoteError>;

    /// Moves the file `id` from the folder `from_parent` to `to_parent`, named `name`.
    fn move_file(
        &mut self,
        id: &str,
        from_parent: &str,
        to_parent: &str,
        name: &str,
    ) -> Result<RemoteFile, RemoteError>;

    /// Creates the folder `name` in `parent`, or at the top of My Drive for `None`.
    /// Returns its id.
    fn create_folder(&mut self, parent: Option<&str>, name: &str) -> Result<String, RemoteError>;

    /// Sends the file or folder `id` to Drive's trash, with what a folder has.
    fn trash(&mut self, id: &str) -> Result<(), RemoteError>;
}
