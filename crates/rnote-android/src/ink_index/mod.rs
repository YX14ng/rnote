//! Searching what was written by hand: the writing grouped into lines and fragments, the
//! local index of what the recognizer read in them, and the search over it.

pub(crate) mod fragments;
pub(crate) mod search;
pub(crate) mod store;

#[cfg(test)]
mod tests;
