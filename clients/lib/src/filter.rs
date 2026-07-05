//! Card filtering logic shared across BlazeList clients.
//!
//! Both the native CLI and WASM web client apply the same filtering
//! pipeline: linked card filter → blaze status → search query → tag
//! selection (AND/OR).

use std::collections::HashSet;

use blazelist_protocol::CardFilter;
use blazelist_protocol::{Card, Entity, Tag};
use chrono::{NaiveDate, Utc};
use uuid::Uuid;

/// Due date filter: which cards to show based on their due date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DueDateFilter {
    /// Show all cards regardless of due date.
    All,
    /// Show only cards with due date before today.
    Overdue,
    /// Show only cards due today.
    Today,
    /// Show cards due today and all future due dates.
    TodayAndUpcoming,
    /// Show only cards due tomorrow.
    UpcomingTomorrow,
    /// Show only cards due within the next 7 days (inclusive of today).
    UpcomingWeek,
    /// Show only cards due within the next 14 days (inclusive of today).
    UpcomingTwoWeeks,
}

impl DueDateFilter {
    pub fn label(&self) -> &str {
        match self {
            Self::All => "All",
            Self::Overdue => "Overdue",
            Self::Today => "Today",
            Self::TodayAndUpcoming => "Today & upcoming",
            Self::UpcomingTomorrow => "Tomorrow",
            Self::UpcomingWeek => "Next 7 days",
            Self::UpcomingTwoWeeks => "Next 14 days",
        }
    }

    /// Returns `true` if this is any upcoming variant (including sub-ranges).
    pub fn is_upcoming(self) -> bool {
        matches!(
            self,
            Self::TodayAndUpcoming
                | Self::UpcomingTomorrow
                | Self::UpcomingWeek
                | Self::UpcomingTwoWeeks
        )
    }

    /// URL value for the filter. `None` when the filter is the default (All)
    /// and can be omitted from the query string.
    pub fn url_value(self) -> Option<&'static str> {
        match self {
            Self::All => None, // default — omit from URL
            Self::Overdue => Some("overdue"),
            Self::Today => Some("today"),
            Self::TodayAndUpcoming => Some("today-upcoming"),
            Self::UpcomingTomorrow => Some("upcoming-tomorrow"),
            Self::UpcomingWeek => Some("upcoming-week"),
            Self::UpcomingTwoWeeks => Some("upcoming-2weeks"),
        }
    }

    pub fn from_url_value(s: &str) -> Self {
        match s {
            "overdue" => Self::Overdue,
            "today" => Self::Today,
            // `upcoming` is a legacy alias kept for old/shared URLs.
            "today-upcoming" | "upcoming" => Self::TodayAndUpcoming,
            "upcoming-tomorrow" => Self::UpcomingTomorrow,
            "upcoming-week" => Self::UpcomingWeek,
            "upcoming-2weeks" => Self::UpcomingTwoWeeks,
            _ => Self::All,
        }
    }
}

/// Tag filter mode.
///
/// - [`Or`](Self::Or): include cards with **any** selected tag.
/// - [`And`](Self::And): include cards with **all** selected tags.
/// - [`Nor`](Self::Nor): exclude cards with **any** selected tag (keep the rest).
/// - [`Nand`](Self::Nand): exclude cards with **all** selected tags (keep the rest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagFilterMode {
    Or,
    And,
    Nor,
    Nand,
}

impl TagFilterMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Or => "OR",
            Self::And => "AND",
            Self::Nor => "NOR",
            Self::Nand => "NAND",
        }
    }

    /// Human-readable explanation of the mode's semantics.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Or => "Any selected tag",
            Self::And => "All selected tags",
            Self::Nor => "Exclude any selected tag",
            Self::Nand => "Exclude all selected tags",
        }
    }

    /// All variants in display order.
    pub const ALL: &'static [TagFilterMode] = &[Self::Or, Self::And, Self::Nor, Self::Nand];

    /// URL value for the mode. `None` when the mode is the default (OR) and
    /// can be omitted from the query string.
    pub fn url_value(&self) -> Option<&'static str> {
        match self {
            Self::Or => None,
            Self::And => Some("and"),
            Self::Nor => Some("nor"),
            Self::Nand => Some("nand"),
        }
    }

    pub fn from_url_value(s: &str) -> Self {
        match s {
            "and" => Self::And,
            "nor" => Self::Nor,
            "nand" => Self::Nand,
            _ => Self::Or,
        }
    }

    /// Whether this mode is compatible with the "no tags" filter.
    /// Only OR is additive; the others select a subset that doesn't
    /// meaningfully combine with "untagged-only".
    pub fn allows_no_tags(&self) -> bool {
        matches!(self, Self::Or)
    }

    /// Returns the next variant, cycling through all four modes:
    /// OR → AND → NOR → NAND → OR.
    pub fn next(&self) -> Self {
        match self {
            Self::Or => Self::And,
            Self::And => Self::Nor,
            Self::Nor => Self::Nand,
            Self::Nand => Self::Or,
        }
    }
}

/// Apply the linked card filter to a list of cards.
///
/// When `linked_ids` is non-empty, only cards whose UUID is in the list
/// are retained. No-op if `linked_ids` is empty.
pub fn apply_linked_card_filter(cards: &mut Vec<Card>, linked_ids: &[Uuid]) {
    if linked_ids.is_empty() {
        return;
    }
    let set: HashSet<Uuid> = linked_ids.iter().copied().collect();
    cards.retain(|c| set.contains(&c.id()));
}

/// Apply the blaze status filter to a list of cards.
pub fn apply_blaze_filter(cards: &mut Vec<Card>, filter: CardFilter) {
    match filter {
        CardFilter::All => {}
        CardFilter::Blazed => cards.retain(|c| c.blazed()),
        CardFilter::Extinguished => cards.retain(|c| !c.blazed()),
    }
}

/// Apply a search query filter (case-insensitive content match, optionally
/// including tag names).
///
/// When `search_tags` is `true`, a card also matches if any of its tags'
/// titles contain the query. The "no tags" special tag is excluded.
///
/// No-op if `query` is empty.
pub fn apply_search_filter(
    cards: &mut Vec<Card>,
    query: &str,
    search_tags: bool,
    all_tags: &[Tag],
) {
    if query.is_empty() {
        return;
    }
    let q = query.to_lowercase();
    cards.retain(|c| {
        if c.content().to_lowercase().contains(&q) {
            return true;
        }
        if search_tags {
            for tag_id in c.tags() {
                if let Some(tag) = all_tags.iter().find(|t| t.id() == *tag_id)
                    && tag.title().to_lowercase().contains(&q)
                {
                    return true;
                }
            }
        }
        false
    });
}

/// Apply a tag filter using the given [`TagFilterMode`], optionally
/// including cards with no tags.
///
/// When `no_tags` is true and `selected_tags` is empty, only untagged cards
/// are shown. When `no_tags` is true and tags are also selected (OR mode),
/// untagged cards are included alongside cards matching the selected tags.
/// The UI prevents combining `no_tags` with non-OR modes — see
/// [`TagFilterMode::allows_no_tags`]. No-op if both `selected_tags` is
/// empty and `no_tags` is false.
pub fn apply_tag_filter(
    cards: &mut Vec<Card>,
    selected_tags: &[Uuid],
    mode: TagFilterMode,
    no_tags: bool,
) {
    if selected_tags.is_empty() && !no_tags {
        return;
    }
    let set: HashSet<Uuid> = selected_tags.iter().copied().collect();
    cards.retain(|c| {
        if no_tags && c.tags().is_empty() {
            return true;
        }
        if selected_tags.is_empty() {
            return false;
        }
        match mode {
            TagFilterMode::Or => c.tags().iter().any(|t| set.contains(t)),
            TagFilterMode::And => set.iter().all(|t| c.tags().contains(t)),
            TagFilterMode::Nor => !c.tags().iter().any(|t| set.contains(t)),
            TagFilterMode::Nand => !set.iter().all(|t| c.tags().contains(t)),
        }
    });
}

/// Apply a due date filter.
///
/// No-op if `filter` is [`DueDateFilter::All`].
/// When `include_overdue` is `true` and `filter` is not `All` or `Overdue`,
/// cards with a due date before today are also included.
pub fn apply_due_date_filter(cards: &mut Vec<Card>, filter: DueDateFilter, include_overdue: bool) {
    let today = Utc::now().date_naive();
    apply_due_date_filter_with_today(cards, filter, today, include_overdue);
}

/// Apply a due date filter using an explicit `today` date (for testability).
///
/// When `include_overdue` is `true` and `filter` is not `All` or `Overdue`,
/// cards with a due date before today are also retained.
pub fn apply_due_date_filter_with_today(
    cards: &mut Vec<Card>,
    filter: DueDateFilter,
    today: NaiveDate,
    include_overdue: bool,
) {
    let overdue_ok = move |d: NaiveDate| include_overdue && d < today;
    // Per-variant predicate over a card's due date. `All` is a no-op (returns
    // early); every other arm keeps a card when its due date matches the
    // variant's window. The Overdue arm deliberately ignores `overdue_ok`
    // (it is unconditionally `date < today`); the rest fall back to it so an
    // explicit "include overdue" pulls past-due cards into the window. The
    // UpcomingWeek/UpcomingTwoWeeks upper bounds are half-open (`date < end`).
    let keep: Box<dyn Fn(NaiveDate) -> bool> = match filter {
        DueDateFilter::All => return,
        DueDateFilter::Overdue => Box::new(move |date| date < today),
        DueDateFilter::Today => Box::new(move |date| date == today || overdue_ok(date)),
        DueDateFilter::TodayAndUpcoming => Box::new(move |date| date >= today || overdue_ok(date)),
        DueDateFilter::UpcomingTomorrow => {
            let tomorrow = today + chrono::Days::new(1);
            Box::new(move |date| date == tomorrow || overdue_ok(date))
        }
        DueDateFilter::UpcomingWeek => {
            let end = today + chrono::Days::new(7);
            Box::new(move |date| (date >= today && date < end) || overdue_ok(date))
        }
        DueDateFilter::UpcomingTwoWeeks => {
            let end = today + chrono::Days::new(14);
            Box::new(move |date| (date >= today && date < end) || overdue_ok(date))
        }
    };
    cards.retain(|c| c.due_date().is_some_and(|d| keep(d.date_naive())));
}

/// Apply the full filtering pipeline: linked cards → blaze status →
/// search → tags.
///
/// Cards are filtered in-place. This is the canonical filtering sequence
/// used by both CLI and WASM clients.
// Nine cohesive filter inputs applied as one in-place pipeline; bundling them
// into a struct would add indirection without clarifying the call sites. This
// silences clippy::too_many_arguments, which otherwise fails the Lint
// workflow's `cargo clippy -- -D warnings` on this (pre-existing) signature.
#[allow(clippy::too_many_arguments)]
pub fn apply_all_filters(
    cards: &mut Vec<Card>,
    linked_ids: &[Uuid],
    blaze_filter: CardFilter,
    search_query: &str,
    selected_tags: &[Uuid],
    tag_mode: TagFilterMode,
    no_tags: bool,
    search_tags: bool,
    all_tags: &[Tag],
) {
    apply_linked_card_filter(cards, linked_ids);
    apply_blaze_filter(cards, blaze_filter);
    apply_search_filter(cards, search_query, search_tags, all_tags);
    apply_tag_filter(cards, selected_tags, tag_mode, no_tags);
}

/// Card sort order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// Highest priority first (default).
    #[default]
    Priority,
    /// Lowest priority first.
    PriorityReverse,
    /// Most recently modified first.
    ModifiedAt,
    /// Least recently modified first.
    ModifiedAtReverse,
    /// Most recently created first.
    CreatedAt,
    /// Least recently created first.
    CreatedAtReverse,
    /// Alphabetical by title (A-Z).
    Title,
    /// Reverse alphabetical by title (Z-A).
    TitleReverse,
    /// Earliest due date first (cards without due date last).
    DueDate,
    /// Latest due date first (cards without due date last).
    DueDateReverse,
}

impl SortOrder {
    pub fn is_default(self) -> bool {
        self == Self::default()
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Priority => "Priority",
            Self::PriorityReverse => "Priority (reverse)",
            Self::ModifiedAt => "Last modified",
            Self::ModifiedAtReverse => "Last modified (reverse)",
            Self::CreatedAt => "Created",
            Self::CreatedAtReverse => "Created (reverse)",
            Self::Title => "Title (A-Z)",
            Self::TitleReverse => "Title (Z-A)",
            Self::DueDate => "Due date",
            Self::DueDateReverse => "Due date (reverse)",
        }
    }

    pub fn url_value(self) -> Option<&'static str> {
        match self {
            Self::Priority => None, // default — omit from URL
            Self::PriorityReverse => Some("priority-reverse"),
            Self::ModifiedAt => Some("modified"),
            Self::ModifiedAtReverse => Some("modified-reverse"),
            Self::CreatedAt => Some("created"),
            Self::CreatedAtReverse => Some("created-reverse"),
            Self::Title => Some("title"),
            Self::TitleReverse => Some("title-reverse"),
            Self::DueDate => Some("due"),
            Self::DueDateReverse => Some("due-reverse"),
        }
    }

    pub fn from_url_value(s: &str) -> Self {
        match s {
            "priority-reverse" => Self::PriorityReverse,
            "modified" => Self::ModifiedAt,
            "modified-reverse" => Self::ModifiedAtReverse,
            "created" => Self::CreatedAt,
            "created-reverse" => Self::CreatedAtReverse,
            "title" => Self::Title,
            "title-reverse" => Self::TitleReverse,
            "due" => Self::DueDate,
            "due-reverse" => Self::DueDateReverse,
            _ => Self::default(),
        }
    }

    /// All variants in display order.
    pub const ALL: &'static [SortOrder] = &[
        Self::Priority,
        Self::PriorityReverse,
        Self::ModifiedAt,
        Self::ModifiedAtReverse,
        Self::CreatedAt,
        Self::CreatedAtReverse,
        Self::Title,
        Self::TitleReverse,
        Self::DueDate,
        Self::DueDateReverse,
    ];
}

/// Sort cards by priority descending (highest priority first).
///
/// This is the standard display order used by both clients.
pub fn sort_by_priority(cards: &mut [Card]) {
    sort_cards(cards, SortOrder::Priority);
}

/// Sort cards according to the given [`SortOrder`].
pub fn sort_cards(cards: &mut [Card], order: SortOrder) {
    match order {
        SortOrder::Priority => {
            cards.sort_unstable_by_key(|c| std::cmp::Reverse(c.priority()));
        }
        SortOrder::PriorityReverse => {
            cards.sort_unstable_by_key(|c| c.priority());
        }
        SortOrder::ModifiedAt => {
            cards.sort_unstable_by_key(|c| std::cmp::Reverse(c.modified_at()));
        }
        SortOrder::ModifiedAtReverse => {
            cards.sort_unstable_by_key(|c| c.modified_at());
        }
        SortOrder::CreatedAt => {
            cards.sort_unstable_by_key(|c| std::cmp::Reverse(c.created_at()));
        }
        SortOrder::CreatedAtReverse => {
            cards.sort_unstable_by_key(|c| c.created_at());
        }
        SortOrder::Title => {
            cards.sort_unstable_by_key(|c| c.content().to_lowercase());
        }
        SortOrder::TitleReverse => {
            cards.sort_unstable_by_key(|c| std::cmp::Reverse(c.content().to_lowercase()));
        }
        SortOrder::DueDate | SortOrder::DueDateReverse => {
            // Only the dated (Some/Some) comparison direction flips; undated
            // cards stay LAST and the priority-descending tiebreaker stays
            // FIXED in both directions.
            let reverse = order == SortOrder::DueDateReverse;
            cards.sort_by(|a, b| {
                let cmp = match (a.due_date(), b.due_date()) {
                    (Some(a_d), Some(b_d)) => {
                        if reverse {
                            b_d.cmp(&a_d)
                        } else {
                            a_d.cmp(&b_d)
                        }
                    }
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                };
                cmp.then_with(|| b.priority().cmp(&a.priority()))
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{fixed_time, fixed_uuid, priority};
    use chrono::{DateTime, Utc};

    fn sample_cards() -> Vec<Card> {
        vec![
            Card::first(
                fixed_uuid(1),
                "Buy groceries".into(),
                priority(3000),
                vec![fixed_uuid(10)],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(2),
                "Write tests".into(),
                priority(2000),
                vec![fixed_uuid(10), fixed_uuid(11)],
                true,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(3),
                "Deploy app".into(),
                priority(1000),
                vec![fixed_uuid(11)],
                false,
                fixed_time(),
                None,
            ),
        ]
    }

    #[test]
    fn tag_filter_mode_label() {
        assert_eq!(TagFilterMode::Or.label(), "OR");
        assert_eq!(TagFilterMode::And.label(), "AND");
        assert_eq!(TagFilterMode::Nor.label(), "NOR");
        assert_eq!(TagFilterMode::Nand.label(), "NAND");
    }

    #[test]
    fn tag_filter_mode_url_roundtrip() {
        for &mode in TagFilterMode::ALL {
            if let Some(val) = mode.url_value() {
                assert_eq!(TagFilterMode::from_url_value(val), mode);
            }
        }
        // Default (OR) has no URL value
        assert_eq!(TagFilterMode::Or.url_value(), None);
        // Unknown string falls back to OR
        assert_eq!(TagFilterMode::from_url_value("nonsense"), TagFilterMode::Or);
    }

    #[test]
    fn tag_filter_mode_allows_no_tags() {
        assert!(TagFilterMode::Or.allows_no_tags());
        assert!(!TagFilterMode::And.allows_no_tags());
        assert!(!TagFilterMode::Nor.allows_no_tags());
        assert!(!TagFilterMode::Nand.allows_no_tags());
    }

    #[test]
    fn tag_filter_mode_next_cycles_all_four() {
        // Full cycle: OR → AND → NOR → NAND → OR.
        let mut mode = TagFilterMode::Or;
        mode = mode.next();
        assert_eq!(mode, TagFilterMode::And);
        mode = mode.next();
        assert_eq!(mode, TagFilterMode::Nor);
        mode = mode.next();
        assert_eq!(mode, TagFilterMode::Nand);
        mode = mode.next();
        assert_eq!(mode, TagFilterMode::Or);
    }

    #[test]
    fn blaze_filter_all() {
        let mut cards = sample_cards();
        apply_blaze_filter(&mut cards, CardFilter::All);
        assert_eq!(cards.len(), 3);
    }

    #[test]
    fn blaze_filter_blazed() {
        let mut cards = sample_cards();
        apply_blaze_filter(&mut cards, CardFilter::Blazed);
        assert_eq!(cards.len(), 1);
        assert!(cards[0].blazed());
    }

    #[test]
    fn blaze_filter_extinguished() {
        let mut cards = sample_cards();
        apply_blaze_filter(&mut cards, CardFilter::Extinguished);
        assert_eq!(cards.len(), 2);
        assert!(cards.iter().all(|c| !c.blazed()));
    }

    #[test]
    fn search_filter_matches() {
        let mut cards = sample_cards();
        apply_search_filter(&mut cards, "groceries", false, &[]);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].content(), "Buy groceries");
    }

    #[test]
    fn search_filter_case_insensitive() {
        let mut cards = sample_cards();
        apply_search_filter(&mut cards, "DEPLOY", false, &[]);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].content(), "Deploy app");
    }

    #[test]
    fn search_filter_empty_query_noop() {
        let mut cards = sample_cards();
        apply_search_filter(&mut cards, "", false, &[]);
        assert_eq!(cards.len(), 3);
    }

    #[test]
    fn tag_filter_or_mode() {
        let mut cards = sample_cards();
        apply_tag_filter(&mut cards, &[fixed_uuid(10)], TagFilterMode::Or, false);
        // Cards 1 and 2 have tag 10
        assert_eq!(cards.len(), 2);
    }

    #[test]
    fn tag_filter_and_mode() {
        let mut cards = sample_cards();
        apply_tag_filter(
            &mut cards,
            &[fixed_uuid(10), fixed_uuid(11)],
            TagFilterMode::And,
            false,
        );
        // Only card 2 has both tags
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].content(), "Write tests");
    }

    #[test]
    fn tag_filter_nor_mode() {
        let mut cards = sample_cards();
        // Exclude any card with tag 10 → only "Deploy app" (tag 11 only) remains.
        apply_tag_filter(&mut cards, &[fixed_uuid(10)], TagFilterMode::Nor, false);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].content(), "Deploy app");
    }

    #[test]
    fn tag_filter_nor_mode_keeps_untagged() {
        let mut cards = sample_cards();
        cards.push(Card::first(
            fixed_uuid(4),
            "Untagged card".into(),
            priority(500),
            vec![],
            false,
            fixed_time(),
            None,
        ));
        apply_tag_filter(&mut cards, &[fixed_uuid(10)], TagFilterMode::Nor, false);
        let names: Vec<_> = cards.iter().map(|c| c.content()).collect();
        assert!(names.contains(&"Deploy app"));
        assert!(names.contains(&"Untagged card"));
        assert!(!names.contains(&"Buy groceries"));
        assert!(!names.contains(&"Write tests"));
    }

    #[test]
    fn tag_filter_nand_mode() {
        let mut cards = sample_cards();
        // Exclude only cards that have BOTH tag 10 AND tag 11 (i.e. card 2).
        apply_tag_filter(
            &mut cards,
            &[fixed_uuid(10), fixed_uuid(11)],
            TagFilterMode::Nand,
            false,
        );
        let names: Vec<_> = cards.iter().map(|c| c.content()).collect();
        assert_eq!(cards.len(), 2);
        assert!(names.contains(&"Buy groceries"));
        assert!(names.contains(&"Deploy app"));
        assert!(!names.contains(&"Write tests"));
    }

    #[test]
    fn tag_filter_nand_single_tag_matches_nor() {
        // With exactly one selected tag, NOR and NAND are equivalent
        // (the "all" set is the "any" set).
        let mut cards_nor = sample_cards();
        let mut cards_nand = sample_cards();
        apply_tag_filter(&mut cards_nor, &[fixed_uuid(11)], TagFilterMode::Nor, false);
        apply_tag_filter(
            &mut cards_nand,
            &[fixed_uuid(11)],
            TagFilterMode::Nand,
            false,
        );
        let nor: Vec<_> = cards_nor.iter().map(|c| c.content()).collect();
        let nand: Vec<_> = cards_nand.iter().map(|c| c.content()).collect();
        assert_eq!(nor, nand);
    }

    #[test]
    fn tag_filter_empty_tags_noop() {
        let mut cards = sample_cards();
        apply_tag_filter(&mut cards, &[], TagFilterMode::Or, false);
        assert_eq!(cards.len(), 3);
    }

    #[test]
    fn tag_filter_no_tags_only() {
        let mut cards = sample_cards();
        // Add a card with no tags
        cards.push(Card::first(
            fixed_uuid(4),
            "Untagged card".into(),
            priority(500),
            vec![],
            false,
            fixed_time(),
            None,
        ));
        apply_tag_filter(&mut cards, &[], TagFilterMode::Or, true);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].content(), "Untagged card");
    }

    #[test]
    fn tag_filter_no_tags_with_selected_tags() {
        let mut cards = sample_cards();
        cards.push(Card::first(
            fixed_uuid(4),
            "Untagged card".into(),
            priority(500),
            vec![],
            false,
            fixed_time(),
            None,
        ));
        // no_tags=true + selected tag 10 → untagged cards + cards with tag 10
        apply_tag_filter(&mut cards, &[fixed_uuid(10)], TagFilterMode::Or, true);
        assert_eq!(cards.len(), 3);
        let names: Vec<_> = cards.iter().map(|c| c.content()).collect();
        assert!(names.contains(&"Buy groceries"));
        assert!(names.contains(&"Write tests"));
        assert!(names.contains(&"Untagged card"));
    }

    #[test]
    fn apply_all_filters_combined() {
        let mut cards = sample_cards();
        // No linked filter + Extinguished + search "app" + no tag filter
        apply_all_filters(
            &mut cards,
            &[],
            CardFilter::Extinguished,
            "app",
            &[],
            TagFilterMode::Or,
            false,
            false,
            &[],
        );
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].content(), "Deploy app");
    }

    #[test]
    fn linked_card_filter_empty_noop() {
        let mut cards = sample_cards();
        apply_linked_card_filter(&mut cards, &[]);
        assert_eq!(cards.len(), 3);
    }

    #[test]
    fn linked_card_filter_retains_matching() {
        let mut cards = sample_cards();
        apply_linked_card_filter(&mut cards, &[fixed_uuid(1), fixed_uuid(3)]);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].content(), "Buy groceries");
        assert_eq!(cards[1].content(), "Deploy app");
    }

    #[test]
    fn linked_card_filter_no_match() {
        let mut cards = sample_cards();
        apply_linked_card_filter(&mut cards, &[fixed_uuid(99)]);
        assert!(cards.is_empty());
    }

    #[test]
    fn apply_all_filters_with_linked_ids() {
        let mut cards = sample_cards();
        // Linked to cards 1 and 2 + All blaze filter + no search + no tags
        apply_all_filters(
            &mut cards,
            &[fixed_uuid(1), fixed_uuid(2)],
            CardFilter::All,
            "",
            &[],
            TagFilterMode::Or,
            false,
            false,
            &[],
        );
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].content(), "Buy groceries");
        assert_eq!(cards[1].content(), "Write tests");
    }

    #[test]
    fn sort_order_default_is_priority() {
        assert_eq!(SortOrder::default(), SortOrder::Priority);
        assert!(SortOrder::Priority.is_default());
        assert!(!SortOrder::Title.is_default());
    }

    #[test]
    fn sort_order_url_roundtrip() {
        for &order in SortOrder::ALL {
            if let Some(val) = order.url_value() {
                assert_eq!(SortOrder::from_url_value(val), order);
            }
        }
        // Default has no URL value
        assert_eq!(SortOrder::Priority.url_value(), None);
        // Unknown string → default
        assert_eq!(SortOrder::from_url_value("nonsense"), SortOrder::Priority);
    }

    #[test]
    fn sort_order_labels_unique() {
        let labels: Vec<_> = SortOrder::ALL.iter().map(|o| o.label()).collect();
        for (i, l) in labels.iter().enumerate() {
            assert!(!l.is_empty(), "label for {:?} is empty", SortOrder::ALL[i]);
        }
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(unique.len(), labels.len(), "duplicate labels");
    }

    #[test]
    fn sort_cards_priority_reverse() {
        let mut cards = vec![
            Card::first(
                fixed_uuid(1),
                "low".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(2),
                "high".into(),
                priority(9000),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(3),
                "mid".into(),
                priority(5000),
                vec![],
                false,
                fixed_time(),
                None,
            ),
        ];
        sort_cards(&mut cards, SortOrder::PriorityReverse);
        assert_eq!(cards[0].content(), "low");
        assert_eq!(cards[1].content(), "mid");
        assert_eq!(cards[2].content(), "high");
    }

    fn time_millis(ms: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(ms).unwrap()
    }

    #[test]
    fn sort_cards_modified_at() {
        let mut cards = vec![
            Card::first(
                fixed_uuid(1),
                "old".into(),
                priority(100),
                vec![],
                false,
                time_millis(1000),
                None,
            ),
            Card::first(
                fixed_uuid(2),
                "new".into(),
                priority(200),
                vec![],
                false,
                time_millis(3000),
                None,
            ),
            Card::first(
                fixed_uuid(3),
                "mid".into(),
                priority(300),
                vec![],
                false,
                time_millis(2000),
                None,
            ),
        ];
        sort_cards(&mut cards, SortOrder::ModifiedAt);
        assert_eq!(cards[0].content(), "new");
        assert_eq!(cards[1].content(), "mid");
        assert_eq!(cards[2].content(), "old");
    }

    #[test]
    fn sort_cards_modified_at_reverse() {
        let mut cards = vec![
            Card::first(
                fixed_uuid(1),
                "old".into(),
                priority(100),
                vec![],
                false,
                time_millis(1000),
                None,
            ),
            Card::first(
                fixed_uuid(2),
                "new".into(),
                priority(200),
                vec![],
                false,
                time_millis(3000),
                None,
            ),
            Card::first(
                fixed_uuid(3),
                "mid".into(),
                priority(300),
                vec![],
                false,
                time_millis(2000),
                None,
            ),
        ];
        sort_cards(&mut cards, SortOrder::ModifiedAtReverse);
        assert_eq!(cards[0].content(), "old");
        assert_eq!(cards[1].content(), "mid");
        assert_eq!(cards[2].content(), "new");
    }

    #[test]
    fn sort_cards_title() {
        let mut cards = vec![
            Card::first(
                fixed_uuid(1),
                "Banana".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(2),
                "Apple".into(),
                priority(200),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(3),
                "cherry".into(),
                priority(300),
                vec![],
                false,
                fixed_time(),
                None,
            ),
        ];
        sort_cards(&mut cards, SortOrder::Title);
        assert_eq!(cards[0].content(), "Apple");
        assert_eq!(cards[1].content(), "Banana");
        assert_eq!(cards[2].content(), "cherry");
    }

    #[test]
    fn sort_cards_title_reverse() {
        let mut cards = vec![
            Card::first(
                fixed_uuid(1),
                "Banana".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(2),
                "Apple".into(),
                priority(200),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(3),
                "cherry".into(),
                priority(300),
                vec![],
                false,
                fixed_time(),
                None,
            ),
        ];
        sort_cards(&mut cards, SortOrder::TitleReverse);
        assert_eq!(cards[0].content(), "cherry");
        assert_eq!(cards[1].content(), "Banana");
        assert_eq!(cards[2].content(), "Apple");
    }

    #[test]
    fn sort_by_priority_descending() {
        let mut cards = vec![
            Card::first(
                fixed_uuid(1),
                "low".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(2),
                "high".into(),
                priority(9000),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(3),
                "mid".into(),
                priority(5000),
                vec![],
                false,
                fixed_time(),
                None,
            ),
        ];
        sort_by_priority(&mut cards);
        assert_eq!(cards[0].content(), "high");
        assert_eq!(cards[1].content(), "mid");
        assert_eq!(cards[2].content(), "low");
    }

    #[test]
    fn sort_cards_due_date() {
        let mut cards = vec![
            Card::first(
                fixed_uuid(1),
                "no due low".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(5),
                "no due high".into(),
                priority(500),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(2),
                "early".into(),
                priority(200),
                vec![],
                false,
                fixed_time(),
                Some(time_millis(1000)),
            ),
            Card::first(
                fixed_uuid(3),
                "late".into(),
                priority(300),
                vec![],
                false,
                fixed_time(),
                Some(time_millis(3000)),
            ),
            Card::first(
                fixed_uuid(4),
                "mid low".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                Some(time_millis(2000)),
            ),
            Card::first(
                fixed_uuid(6),
                "mid high".into(),
                priority(400),
                vec![],
                false,
                fixed_time(),
                Some(time_millis(2000)),
            ),
        ];
        sort_cards(&mut cards, SortOrder::DueDate);
        assert_eq!(cards[0].content(), "early");
        assert_eq!(cards[1].content(), "mid high");
        assert_eq!(cards[2].content(), "mid low");
        assert_eq!(cards[3].content(), "late");
        // No due date cards sorted by priority (highest first)
        assert_eq!(cards[4].content(), "no due high");
        assert_eq!(cards[5].content(), "no due low");
    }

    #[test]
    fn sort_cards_due_date_reverse() {
        let mut cards = vec![
            Card::first(
                fixed_uuid(1),
                "no due low".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(5),
                "no due high".into(),
                priority(500),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(2),
                "early".into(),
                priority(200),
                vec![],
                false,
                fixed_time(),
                Some(time_millis(1000)),
            ),
            Card::first(
                fixed_uuid(3),
                "late".into(),
                priority(300),
                vec![],
                false,
                fixed_time(),
                Some(time_millis(3000)),
            ),
            Card::first(
                fixed_uuid(4),
                "mid low".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                Some(time_millis(2000)),
            ),
            Card::first(
                fixed_uuid(6),
                "mid high".into(),
                priority(400),
                vec![],
                false,
                fixed_time(),
                Some(time_millis(2000)),
            ),
        ];
        sort_cards(&mut cards, SortOrder::DueDateReverse);
        assert_eq!(cards[0].content(), "late");
        assert_eq!(cards[1].content(), "mid high");
        assert_eq!(cards[2].content(), "mid low");
        assert_eq!(cards[3].content(), "early");
        // No due date cards sorted by priority (highest first)
        assert_eq!(cards[4].content(), "no due high");
        assert_eq!(cards[5].content(), "no due low");
    }

    // ---- Due date filter tests ----

    fn due_date_cards(today: NaiveDate) -> Vec<Card> {
        use chrono::Days;
        let to_dt = |d: NaiveDate| -> DateTime<Utc> { d.and_hms_opt(12, 0, 0).unwrap().and_utc() };
        vec![
            Card::first(
                fixed_uuid(1),
                "yesterday".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                Some(to_dt(today - Days::new(1))),
            ),
            Card::first(
                fixed_uuid(2),
                "today".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                Some(to_dt(today)),
            ),
            Card::first(
                fixed_uuid(3),
                "tomorrow".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                Some(to_dt(today + Days::new(1))),
            ),
            Card::first(
                fixed_uuid(4),
                "in3days".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                Some(to_dt(today + Days::new(3))),
            ),
            Card::first(
                fixed_uuid(5),
                "in7days".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                Some(to_dt(today + Days::new(7))),
            ),
            Card::first(
                fixed_uuid(6),
                "in10days".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                Some(to_dt(today + Days::new(10))),
            ),
            Card::first(
                fixed_uuid(7),
                "in14days".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                Some(to_dt(today + Days::new(14))),
            ),
            Card::first(
                fixed_uuid(8),
                "in20days".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                Some(to_dt(today + Days::new(20))),
            ),
            Card::first(
                fixed_uuid(9),
                "no_due".into(),
                priority(100),
                vec![],
                false,
                fixed_time(),
                None,
            ),
        ]
    }

    fn names(cards: &[Card]) -> Vec<&str> {
        cards.iter().map(|c| c.content()).collect()
    }

    #[test]
    fn due_filter_upcoming_tomorrow() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let mut cards = due_date_cards(today);
        apply_due_date_filter_with_today(&mut cards, DueDateFilter::UpcomingTomorrow, today, false);
        let n = names(&cards);
        assert_eq!(n, vec!["tomorrow"]);
    }

    #[test]
    fn due_filter_upcoming_week() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let mut cards = due_date_cards(today);
        apply_due_date_filter_with_today(&mut cards, DueDateFilter::UpcomingWeek, today, false);
        let n = names(&cards);
        assert!(n.contains(&"today"));
        assert!(n.contains(&"tomorrow"));
        assert!(n.contains(&"in3days"));
        assert!(!n.contains(&"in7days")); // day 7 is outside the 7-day window
        assert!(!n.contains(&"in10days"));
        assert!(!n.contains(&"yesterday"));
        assert!(!n.contains(&"no_due"));
    }

    #[test]
    fn due_filter_upcoming_two_weeks() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let mut cards = due_date_cards(today);
        apply_due_date_filter_with_today(&mut cards, DueDateFilter::UpcomingTwoWeeks, today, false);
        let n = names(&cards);
        assert!(n.contains(&"today"));
        assert!(n.contains(&"tomorrow"));
        assert!(n.contains(&"in3days"));
        assert!(n.contains(&"in7days"));
        assert!(n.contains(&"in10days"));
        assert!(!n.contains(&"in14days")); // day 14 is outside the 14-day window
        assert!(!n.contains(&"in20days"));
        assert!(!n.contains(&"yesterday"));
        assert!(!n.contains(&"no_due"));
    }

    #[test]
    fn due_filter_include_overdue_with_today() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let mut cards = due_date_cards(today);
        apply_due_date_filter_with_today(&mut cards, DueDateFilter::Today, today, true);
        let n = names(&cards);
        assert!(n.contains(&"yesterday"));
        assert!(n.contains(&"today"));
        assert!(!n.contains(&"tomorrow"));
        assert!(!n.contains(&"no_due"));
    }

    #[test]
    fn due_filter_include_overdue_with_upcoming_week() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let mut cards = due_date_cards(today);
        apply_due_date_filter_with_today(&mut cards, DueDateFilter::UpcomingWeek, today, true);
        let n = names(&cards);
        assert!(n.contains(&"yesterday"));
        assert!(n.contains(&"today"));
        assert!(n.contains(&"tomorrow"));
        assert!(n.contains(&"in3days"));
        assert!(!n.contains(&"in7days")); // day 7 is outside the 7-day window
        assert!(!n.contains(&"in10days"));
        assert!(!n.contains(&"no_due"));
    }

    #[test]
    fn due_filter_today_and_upcoming() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let mut cards = due_date_cards(today);
        apply_due_date_filter_with_today(&mut cards, DueDateFilter::TodayAndUpcoming, today, false);
        let n = names(&cards);
        assert!(n.contains(&"today"));
        assert!(n.contains(&"tomorrow"));
        assert!(n.contains(&"in20days"));
        assert!(!n.contains(&"yesterday"));
        assert!(!n.contains(&"no_due"));
    }

    #[test]
    fn due_filter_today_and_upcoming_with_overdue() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let mut cards = due_date_cards(today);
        apply_due_date_filter_with_today(&mut cards, DueDateFilter::TodayAndUpcoming, today, true);
        let n = names(&cards);
        assert!(n.contains(&"yesterday"));
        assert!(n.contains(&"today"));
        assert!(n.contains(&"tomorrow"));
        assert!(n.contains(&"in20days"));
        assert!(!n.contains(&"no_due"));
    }

    #[test]
    fn due_filter_overdue_only_past() {
        // The Overdue arm is unconditionally `date < today`: only past-due
        // cards survive, and today/tomorrow/future/no-due are all excluded.
        let today = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let mut cards = due_date_cards(today);
        apply_due_date_filter_with_today(&mut cards, DueDateFilter::Overdue, today, false);
        let n = names(&cards);
        assert_eq!(n, vec!["yesterday"]);
    }

    #[test]
    fn due_filter_overdue_ignores_include_overdue_flag() {
        // The Overdue arm deliberately ignores `include_overdue`: passing
        // `true` must NOT additionally pull in today or any future cards.
        // Overdue stays past-only regardless of the flag.
        let today = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let mut cards = due_date_cards(today);
        apply_due_date_filter_with_today(&mut cards, DueDateFilter::Overdue, today, true);
        // The exact-equality assert proves no today/future/no-due card
        // leaked in: "yesterday" is the whole surviving set.
        assert_eq!(names(&cards), vec!["yesterday"]);
    }

    // ---- Search filter with tag-title matching ----

    #[test]
    fn search_filter_matches_via_tag_title() {
        // Card 1 has tag 10 ("urgent"); card 3 has tag 11 ("personal").
        // Searching for "urgent" with search_tags=true must match the card
        // via its tag title even though no card's content contains "urgent".
        let all_tags = vec![
            Tag::first(fixed_uuid(10), "urgent".into(), None, fixed_time()),
            Tag::first(fixed_uuid(11), "personal".into(), None, fixed_time()),
        ];
        let mut cards = sample_cards();
        apply_search_filter(&mut cards, "urgent", true, &all_tags);
        // Cards 1 and 2 carry tag 10.
        assert_eq!(cards.len(), 2);
        let n = names(&cards);
        assert!(n.contains(&"Buy groceries"));
        assert!(n.contains(&"Write tests"));
        assert!(!n.contains(&"Deploy app"));
    }

    #[test]
    fn search_filter_tag_title_case_insensitive() {
        // Tag-title matching lowercases both sides, so an uppercase query
        // still matches a lowercase tag title.
        let all_tags = vec![Tag::first(
            fixed_uuid(11),
            "personal".into(),
            None,
            fixed_time(),
        )];
        let mut cards = sample_cards();
        apply_search_filter(&mut cards, "PERSONAL", true, &all_tags);
        // Cards 2 and 3 carry tag 11.
        assert_eq!(cards.len(), 2);
        let n = names(&cards);
        assert!(n.contains(&"Write tests"));
        assert!(n.contains(&"Deploy app"));
    }

    #[test]
    fn search_filter_tag_title_ignored_when_search_tags_false() {
        // The exact same setup with search_tags=false must NOT match via the
        // tag title: only content is consulted, and no content contains
        // "urgent", so every card is dropped.
        let all_tags = vec![Tag::first(
            fixed_uuid(10),
            "urgent".into(),
            None,
            fixed_time(),
        )];
        let mut cards = sample_cards();
        apply_search_filter(&mut cards, "urgent", false, &all_tags);
        assert!(cards.is_empty());
    }

    #[test]
    fn search_filter_excludes_no_tags_card() {
        // The "no tags" special tag is excluded from tag-title search: an
        // untagged card has an empty tags() list, so the tag-search branch
        // never fires for it. With a query that matches neither its content
        // nor any real tag, an untagged card is dropped even when
        // search_tags=true.
        let all_tags = vec![Tag::first(
            fixed_uuid(10),
            "urgent".into(),
            None,
            fixed_time(),
        )];
        let mut cards = vec![Card::first(
            fixed_uuid(4),
            "untitled".into(),
            priority(500),
            vec![],
            false,
            fixed_time(),
            None,
        )];
        apply_search_filter(&mut cards, "urgent", true, &all_tags);
        assert!(cards.is_empty());
    }

    #[test]
    fn search_filter_tag_present_only_in_all_tags_does_not_match() {
        // A tag whose title contains the query but which no card references
        // must not cause a match: the iteration is over each card's own
        // tags(), not over all_tags.
        let all_tags = vec![
            Tag::first(fixed_uuid(10), "work".into(), None, fixed_time()),
            Tag::first(fixed_uuid(11), "home".into(), None, fixed_time()),
            // "urgent" exists as a tag but is not on any card in sample_cards().
            Tag::first(fixed_uuid(12), "urgent".into(), None, fixed_time()),
        ];
        let mut cards = sample_cards();
        apply_search_filter(&mut cards, "urgent", true, &all_tags);
        assert!(cards.is_empty());
    }

    #[test]
    fn search_filter_unknown_card_tag_id_is_skipped() {
        // When a card references a tag id absent from all_tags, the
        // `find(...)` lookup yields None and that tag is silently skipped
        // rather than matching or panicking. Here all_tags is empty, so no
        // card can match via a tag, and content does not contain "urgent".
        let mut cards = sample_cards();
        apply_search_filter(&mut cards, "urgent", true, &[]);
        assert!(cards.is_empty());
    }

    #[test]
    fn search_filter_content_still_matches_with_search_tags() {
        // With search_tags=true the content match is still checked first, so a
        // query matching content matches regardless of tags.
        let all_tags = vec![Tag::first(
            fixed_uuid(10),
            "urgent".into(),
            None,
            fixed_time(),
        )];
        let mut cards = sample_cards();
        apply_search_filter(&mut cards, "groceries", true, &all_tags);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].content(), "Buy groceries");
    }

    #[test]
    fn due_filter_is_upcoming() {
        assert!(!DueDateFilter::All.is_upcoming());
        assert!(!DueDateFilter::Overdue.is_upcoming());
        assert!(!DueDateFilter::Today.is_upcoming());
        assert!(DueDateFilter::TodayAndUpcoming.is_upcoming());
        assert!(DueDateFilter::UpcomingTomorrow.is_upcoming());
        assert!(DueDateFilter::UpcomingWeek.is_upcoming());
        assert!(DueDateFilter::UpcomingTwoWeeks.is_upcoming());
    }

    #[test]
    fn due_filter_url_roundtrip() {
        // Mirrors `tag_filter_mode_url_roundtrip` / `sort_order_url_roundtrip`:
        // every variant with a URL token must parse back to itself.
        let all = [
            DueDateFilter::All,
            DueDateFilter::Overdue,
            DueDateFilter::Today,
            DueDateFilter::TodayAndUpcoming,
            DueDateFilter::UpcomingTomorrow,
            DueDateFilter::UpcomingWeek,
            DueDateFilter::UpcomingTwoWeeks,
        ];
        for filter in all {
            if let Some(val) = filter.url_value() {
                assert_eq!(DueDateFilter::from_url_value(val), filter);
            }
        }
        // Default (All) has no URL value and is what missing/unknown
        // tokens fall back to.
        assert_eq!(DueDateFilter::All.url_value(), None);
        assert_eq!(DueDateFilter::from_url_value(""), DueDateFilter::All);
        assert_eq!(
            DueDateFilter::from_url_value("nonsense"),
            DueDateFilter::All
        );
        // `upcoming` is a deliberate legacy alias for old/shared URLs.
        assert_eq!(
            DueDateFilter::from_url_value("upcoming"),
            DueDateFilter::TodayAndUpcoming
        );
    }

    #[test]
    fn due_filter_labels_non_empty() {
        let variants = [
            DueDateFilter::All,
            DueDateFilter::Overdue,
            DueDateFilter::Today,
            DueDateFilter::TodayAndUpcoming,
            DueDateFilter::UpcomingTomorrow,
            DueDateFilter::UpcomingWeek,
            DueDateFilter::UpcomingTwoWeeks,
        ];
        for v in &variants {
            assert!(!v.label().is_empty(), "label for {:?} is empty", v);
        }
    }
}
