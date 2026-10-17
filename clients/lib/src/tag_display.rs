//! Tag display ordering.
//!
//! Every view renders tags case-insensitively alphabetical by title, and no
//! tag list arrives that way: a card's stored list and `TagGraph::closure_of`
//! are both UUID-ordered, and a selection the user builds up in the editor or
//! the filter bar is in click order.

use blazelist_protocol::{Entity, Tag};
use rgb::RGB8;
use uuid::Uuid;

/// A tag resolved for rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDisplay {
    pub id: Uuid,
    pub title: String,
    pub color: Option<RGB8>,
}

/// Order tags for display, in place.
pub fn sort_tags(tags: &mut [Tag]) {
    tags.sort_by_cached_key(|t| t.title().to_lowercase());
}

/// Resolve tag IDs against the known tag set, in display order. IDs with no
/// matching tag are dropped — a client can name a tag it has not synced yet,
/// or one deleted elsewhere.
pub fn resolve_tags<I: IntoIterator<Item = Uuid>>(ids: I, all_tags: &[Tag]) -> Vec<TagDisplay> {
    let mut resolved: Vec<TagDisplay> = ids
        .into_iter()
        .filter_map(|id| {
            let tag = all_tags.iter().find(|t| t.id() == id)?;
            Some(TagDisplay {
                id,
                title: tag.title().to_string(),
                color: tag.color(),
            })
        })
        .collect();
    resolved.sort_by_cached_key(|t| t.title.to_lowercase());
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{fixed_time, fixed_uuid};

    fn tag(n: u8, title: &str, color: Option<RGB8>) -> Tag {
        Tag::first(fixed_uuid(n), title.to_string(), color, fixed_time())
    }

    fn titles(tags: &[Tag]) -> Vec<&str> {
        tags.iter().map(|t| t.title()).collect()
    }

    #[test]
    fn sort_tags_ignores_case() {
        let mut tags = vec![
            tag(1, "Zebra", None),
            tag(2, "apple", None),
            tag(3, "Banana", None),
        ];
        sort_tags(&mut tags);
        assert_eq!(titles(&tags), ["apple", "Banana", "Zebra"]);
    }

    #[test]
    fn resolve_tags_orders_alphabetically_not_by_id() {
        let tags = vec![
            tag(1, "Zebra", None),
            tag(2, "apple", None),
            tag(3, "Banana", Some(RGB8::new(192, 120, 48))),
        ];
        let resolved = resolve_tags([fixed_uuid(1), fixed_uuid(2), fixed_uuid(3)], &tags);
        let got: Vec<&str> = resolved.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(got, ["apple", "Banana", "Zebra"]);
        assert_eq!(resolved[1].color, Some(RGB8::new(192, 120, 48)));
    }

    #[test]
    fn resolve_tags_drops_unknown_ids() {
        let resolved = resolve_tags([fixed_uuid(9), fixed_uuid(1)], &[tag(1, "known", None)]);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, fixed_uuid(1));
    }
}
