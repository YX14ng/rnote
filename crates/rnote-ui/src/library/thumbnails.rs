//! Notebook thumbnails for the library cards, rendered off the main thread and cached on
//! disk and in memory.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::{gdk, gio, glib, prelude::*};
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
    /// By notebook path: the version's key and its texture. A newer version replaces the
    /// older one, so editing a notebook doesn't pile up textures.
    textures: Rc<RefCell<HashMap<String, (String, gdk::Texture)>>>,
    /// The thumbnails being rendered, by key, with the pictures waiting for them: a library
    /// shown again meanwhile waits for the same render instead of starting another.
    waiting: Rc<RefCell<HashMap<String, Vec<glib::WeakRef<gtk4::Picture>>>>>,
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
            waiting: Rc::default(),
        }
    }

    /// Shows the thumbnail of `notebook` of the library at `root` in `picture`: right away
    /// when known, else once rendered. On failure the picture keeps its plain cover.
    pub(crate) fn load(&self, picture: &gtk4::Picture, root: PathBuf, notebook: &Notebook) {
        let key = thumbnail_key(notebook);
        if let Some((known, texture)) = self.textures.borrow().get(&notebook.path)
            && *known == key
        {
            picture.set_paintable(Some(texture));
            return;
        }
        if let Some(pictures) = self.waiting.borrow_mut().get_mut(&key) {
            pictures.push(picture.downgrade());
            return;
        }
        self.waiting
            .borrow_mut()
            .insert(key.clone(), vec![picture.downgrade()]);
        let disk = self.disk.clone();
        let notebook = notebook.clone();
        let textures = Rc::clone(&self.textures);
        let waiting = Rc::clone(&self.waiting);
        glib::spawn_future_local(async move {
            let path = notebook.path.clone();
            // Rendered and decoded off the main thread: a library opens with hundreds.
            let rendered = gio::spawn_blocking(move || {
                let png = disk.get_or_render(&root, &notebook, RENDER_SIZE_PX)?;
                Ok::<_, anyhow::Error>(gdk::Texture::from_bytes(&glib::Bytes::from_owned(png))?)
            })
            .await;
            let pictures = waiting.borrow_mut().remove(&key).unwrap_or_default();
            let texture = match rendered {
                Ok(Ok(texture)) => texture,
                Ok(Err(e)) => {
                    debug!("No thumbnail for {path}, Err: {e:?}");
                    return;
                }
                Err(_) => {
                    debug!("Rendering the thumbnail of {path} panicked");
                    return;
                }
            };
            for picture in pictures.iter().filter_map(glib::WeakRef::upgrade) {
                picture.set_paintable(Some(&texture));
            }
            textures.borrow_mut().insert(path, (key, texture));
        });
    }
}
