//! Tags: named colors attached to notebooks.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;

use anyhow::bail;
use serde::Serialize;

use crate::Library;
use crate::meta::TagRecord;
use crate::names::validate_name;

/// A tag.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tag {
    /// Its identifier, stable across renames.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its color, as ARGB.
    pub color: u32,
}

impl Library {
    /// The tags, sorted by name.
    #[must_use]
    pub fn tags(&self) -> Vec<Tag> {
        let mut tags: Vec<_> = self
            .meta
            .tags
            .iter()
            .filter(|(_, record)| !record.deleted)
            .map(|(id, record)| Tag {
                id: id.clone(),
                name: record.name.clone(),
                color: record.color,
            })
            .collect();
        tags.sort_by(|a, b| a.name.cmp(&b.name));
        tags
    }

    /// Creates a tag, returning its id.
    pub fn create_tag(&mut self, name: &str, color: u32) -> anyhow::Result<String> {
        let name = validate_name(name)?;
        let now = self.now();
        let id = loop {
            let id = format!("{:016x}", RandomState::new().hash_one((now, &name)));
            if !self.meta.tags.contains_key(&id) {
                break id;
            }
        };
        self.meta
            .tags
            .insert(id.clone(), TagRecord { name, color, updated_at: now, deleted: false });
        self.save_meta()?;
        Ok(id)
    }

    /// Changes the name and color of a tag.
    pub fn update_tag(&mut self, id: &str, name: &str, color: u32) -> anyhow::Result<()> {
        let name = validate_name(name)?;
        let now = self.now();
        let Some(record) = self.meta.tags.get_mut(id).filter(|record| !record.deleted) else {
            bail!("There is no tag with id {id:?}");
        };
        record.name = name;
        record.color = color;
        record.updated_at = now;
        self.save_meta()
    }

    /// Deletes a tag; notebooks stop showing it.
    pub fn delete_tag(&mut self, id: &str) -> anyhow::Result<()> {
        let now = self.now();
        let Some(record) = self.meta.tags.get_mut(id).filter(|record| !record.deleted) else {
            bail!("There is no tag with id {id:?}");
        };
        record.deleted = true;
        record.updated_at = now;
        self.save_meta()
    }

    /// Sets the tags of the notebook at `path` to `tag_ids`.
    pub fn set_tags(&mut self, path: &str, tag_ids: &[String]) -> anyhow::Result<()> {
        self.existing_notebook(path)?;
        if let Some(unknown) =
            tag_ids.iter().find(|id| !self.meta.tags.get(*id).is_some_and(|tag| !tag.deleted))
        {
            bail!("There is no tag with id {unknown:?}");
        }
        self.meta.set_tags_of(path, tag_ids.to_vec(), self.now());
        self.save_meta()
    }
}
