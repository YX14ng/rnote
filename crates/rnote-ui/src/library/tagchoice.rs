//! Choosing the tags of several notebooks at once, as on Android: a tag is checked when
//! all of them have it, and only the tags the user toggles change.

use std::collections::BTreeSet;

/// The tags every one of the notebooks with `tags` has: the ones checked at first.
pub(crate) fn tags_all_have(tags: &[Vec<String>]) -> BTreeSet<String> {
    let mut notebooks = tags.iter();
    let Some(first) = notebooks.next() else {
        return BTreeSet::new();
    };
    let mut all: BTreeSet<String> = first.iter().cloned().collect();
    for tags in notebooks {
        all.retain(|tag| tags.contains(tag));
    }
    all
}

/// The tags of a notebook that had `current` after the user turned the checked tags from
/// `initially` into `checked`: checked ones are added, unchecked ones that were checked
/// are removed, and the rest stay.
pub(crate) fn apply_tag_choice(
    current: &[String],
    initially: &BTreeSet<String>,
    checked: &BTreeSet<String>,
) -> Vec<String> {
    let mut result: Vec<String> = current
        .iter()
        .filter(|tag| checked.contains(*tag) || !initially.contains(*tag))
        .cloned()
        .collect();
    for tag in checked.difference(initially) {
        if !result.contains(tag) {
            result.push(tag.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{apply_tag_choice, tags_all_have};

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    fn tags(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn tags_all_have_is_the_intersection() {
        let all = tags_all_have(&[tags(&["a", "b"]), tags(&["b", "c"])]);

        assert_eq!(all, set(&["b"]));
    }

    #[test]
    fn checking_a_tag_adds_it_and_keeps_the_others() {
        let result = apply_tag_choice(&tags(&["x"]), &set(&[]), &set(&["a"]));

        assert_eq!(result, tags(&["x", "a"]));
    }

    #[test]
    fn unchecking_a_tag_all_had_removes_it() {
        let result = apply_tag_choice(&tags(&["a", "x"]), &set(&["a"]), &set(&[]));

        assert_eq!(result, tags(&["x"]));
    }

    #[test]
    fn untouched_tags_stay_as_each_notebook_had_them() {
        // "x" was on only some notebooks: unchecked before and after, so it stays.
        let result = apply_tag_choice(&tags(&["x", "a"]), &set(&["a"]), &set(&["a"]));

        assert_eq!(result, tags(&["x", "a"]));
    }

    #[test]
    fn a_tag_already_there_is_not_added_twice() {
        let result = apply_tag_choice(&tags(&["a"]), &set(&[]), &set(&["a"]));

        assert_eq!(result, tags(&["a"]));
    }
}
