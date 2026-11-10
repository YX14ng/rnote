//! Notebook thumbnails for the library cards, rendered off the main thread and cached on
//! disk and in memory.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::{gdk, gio, glib, glib::clone};
use rnote_library::{Notebook, ThumbnailCache, thumbnail_key};
use tracing::debug;

/// The width of a card's thumbnail, in logical pixels.
pub(crate) const THUMBNAIL_WIDTH: i32 = 190;
/// The size thumbnails are rendered at: twice the card, for high density screens.
const RENDER_SIZE_PX: u32 = 2 * THUMBNAIL_WIDTH as u32;

/// Loads thumbnails into pictures, remembering the textures of the open library.
#[derive(Debug, Clone)]
pub(crate) struct Thumbnails {
    disk: ThumbnailCache,
    textures: Rc<RefCell<HashMap<String, gdk::Texture>>>,
}

impl Default for Thumbnails {
    fn default() -> Self {
        Self::new(glib::user_cache_dir().join("rnote").join("thumbnails"))
    }
}

impl Thumbnails {
    /// Thumbnails cached on disk in `dir`.
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self {
            disk: ThumbnailCache::new(dir),
            textures: Rc::default(),
        }
    }

    /// Shows the thumbnail of `notebook` of the library at `root` in `picture`: right away
    /// when known, else once rendered. On failure the picture keeps its plain cover.
    pub(crate) fn load(&self, picture: &gtk4::Picture, root: PathBuf, notebook: &Notebook) {
        let key = thumbnail_key(notebook);
        if let Some(texture) = self.textures.borrow().get(&key) {
            picture.set_paintable(Some(texture));
            return;
        }
        let disk = self.disk.clone();
        let notebook = notebook.clone();
        let textures = Rc::clone(&self.textures);
        glib::spawn_future_local(clone!(
            #[weak]
            picture,
            async move {
                let path = notebook.path.clone();
                let rendered = gio::spawn_blocking(move || {
                    disk.get_or_render(&root, &notebook, RENDER_SIZE_PX)
                })
                .await;
                let png = match rendered {
                    Ok(Ok(png)) => png,
                    Ok(Err(e)) => {
                        debug!("No thumbnail for {path}, Err: {e:?}");
                        return;
                    }
                    Err(_) => {
                        debug!("Rendering the thumbnail of {path} panicked");
                        return;
                    }
                };
                match gdk::Texture::from_bytes(&glib::Bytes::from_owned(png)) {
                    Ok(texture) => {
                        picture.set_paintable(Some(&texture));
                        textures.borrow_mut().insert(key, texture);
                    }
                    Err(e) => debug!("Decoding the thumbnail of {path} failed, Err: {e:?}"),
                }
            }
        ));
    }
}
