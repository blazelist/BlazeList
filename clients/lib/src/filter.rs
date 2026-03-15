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
    /// Show only cards with due date after today (all future).
    Upcoming,
    /// Show only cards due tomorrow.
    UpcomingTomorrow,
    /// Show only cards due within 7 days (exclusive of today).
    UpcomingWeek,
    /// Show only cards due within 14 days (exclusive of today).
    UpcomingTwoWeeks,
}

impl DueDateFilter {
    pub fn label(&self) -> &str {
        match self {
            Self::All => "All",
            Self::Overdue => "Overdue",
            Self::Today => "Today",
            Self::TodayAndUpcoming => "Today & upcoming",
            Self::Upcoming => "All upcoming",
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
                | Self::Upcoming
                | Self::UpcomingTomorrow
                | Self::UpcomingWeek
                | Self::UpcomingTwoWeeks
        )
    }
}

/// Tag filter mode: AND requires all selected tags, OR requires any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagFilterMode {
    Or,
    And,
}

impl TagFilterMode {
    pub fn label(&self) -> &str {
        match self {
            Self::Or => "OR",
            Self::And => "AND",
        }
    }

    pub fn toggle(&self) -> Self {
        match self {
            Self::Or => Self::And,
            Self::And => Self::Or,
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
                if let Some(tag) = all_tags.iter().find(|t| t.id() == *tag_id) {
                    if tag.title().to_lowercase().contains(&q) {
                        return true;
                    }
                }
            }
        }
        false
    });
}

/// Apply a tag filter using the given mode (AND/OR), optionally including
/// cards with no tags.
///
/// When `no_tags` is true and `selected_tags` is empty, only untagged cards
/// are shown. When `no_tags` is true and tags are also selected (OR mode),
/// untagged cards are included alongside cards matching the selected tags.
/// The UI prevents combining `no_tags` with AND mode or with selected tags
/// in AND mode. No-op if both `selected_tags` is empty and `no_tags` is
/// false.
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
        }
    });
}

/// Apply a due date filter.
///
/// No-op if `filter` is [`DueDateFilter::All`].
/// When `include_overdue` is `true` and `filter` is not `All` or `Overdue`,
/// cards with a due date before today are also included.
pub fn apply_due_date_filter(
    cards: &mut Vec<Card>,
    filter: DueDateFilter,
    include_overdue: bool,
) {
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
    let overdue_ok = |d: NaiveDate| include_overdue && d < today;
    match filter {
        DueDateFilter::All => {}
        DueDateFilter::Overdue => {
            cards.retain(|c| {
                c.due_date()
                    .map(|d| d.date_naive() < today)
                    .unwrap_or(false)
            });
        }
        DueDateFilter::Today => {
            cards.retain(|c| {
                c.due_date()
                    .map(|d| {
                        let date = d.date_naive();
                        date == today || overdue_ok(date)
                    })
                    .unwrap_or(false)
            });
        }
        DueDateFilter::TodayAndUpcoming => {
            cards.retain(|c| {
                c.due_date()
                    .map(|d| {
                        let date = d.date_naive();
                        date >= today || overdue_ok(date)
                    })
                    .unwrap_or(false)
            });
        }
        DueDateFilter::Upcoming => {
            cards.retain(|c| {
                c.due_date()
                    .map(|d| {
                        let date = d.date_naive();
                        date > today || overdue_ok(date)
                    })
                    .unwrap_or(false)
            });
        }
        DueDateFilter::UpcomingTomorrow => {
            let tomorrow = today + chrono::Days::new(1);
            cards.retain(|c| {
                c.due_date()
                    .map(|d| {
                        let date = d.date_naive();
                        date == tomorrow || overdue_ok(date)
                    })
                    .unwrap_or(false)
            });
        }
        DueDateFilter::UpcomingWeek => {
            let end = today + chrono::Days::new(7);
            cards.retain(|c| {
                c.due_date()
                    .map(|d| {
                        let date = d.date_naive();
                        (date > today && date <= end) || overdue_ok(date)
                    })
                    .unwrap_or(false)
            });
        }
        DueDateFilter::UpcomingTwoWeeks => {
            let end = today + chrono::Days::new(14);
            cards.retain(|c| {
                c.due_date()
                    .map(|d| {
                        let date = d.date_naive();
                        (date > today && date <= end) || overdue_ok(date)
                    })
                    .unwrap_or(false)
            });
        }
    }
}

/// Apply the full filtering pipeline: linked cards → blaze status →
/// search → tags.
///
/// Cards are filtered in-place. This is the canonical filtering sequence
/// used by both CLI and WASM clients.
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
        SortOrder::DueDate => {
            cards.sort_by(|a, b| {
                let cmp = match (a.due_date(), b.due_date()) {
                    (Some(a_d), Some(b_d)) => a_d.cmp(&b_d),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                };
                cmp.then_with(|| b.priority().cmp(&a.priority()))
            });
        }
        SortOrder::DueDateReverse => {
            cards.sort_by(|a, b| {
                let cmp = match (a.due_date(), b.due_date()) {
                    (Some(a_d), Some(b_d)) => b_d.cmp(&a_d),
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
    }

    #[test]
    fn tag_filter_mode_toggle() {
        assert_eq!(TagFilterMode::Or.toggle(), TagFilterMode::And);
        assert_eq!(TagFilterMode::And.toggle(), TagFilterMode::Or);
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
        let to_dt = |d: NaiveDate| -> DateTime<Utc> {
            d.and_hms_opt(12, 0, 0)
                .unwrap()
                .and_utc()
        };
        vec![
            Card::first(fixed_uuid(1), "yesterday".into(), priority(100), vec![], false, fixed_time(), Some(to_dt(today - Days::new(1)))),
            Card::first(fixed_uuid(2), "today".into(), priority(100), vec![], false, fixed_time(), Some(to_dt(today))),
            Card::first(fixed_uuid(3), "tomorrow".into(), priority(100), vec![], false, fixed_time(), Some(to_dt(today + Days::new(1)))),
            Card::first(fixed_uuid(4), "in3days".into(), priority(100), vec![], false, fixed_time(), Some(to_dt(today + Days::new(3)))),
            Card::first(fixed_uuid(5), "in7days".into(), priority(100), vec![], false, fixed_time(), Some(to_dt(today + Days::new(7)))),
            Card::first(fixed_uuid(6), "in10days".into(), priority(100), vec![], false, fixed_time(), Some(to_dt(today + Days::new(10)))),
            Card::first(fixed_uuid(7), "in14days".into(), priority(100), vec![], false, fixed_time(), Some(to_dt(today + Days::new(14)))),
            Card::first(fixed_uuid(8), "in20days".into(), priority(100), vec![], false, fixed_time(), Some(to_dt(today + Days::new(20)))),
            Card::first(fixed_uuid(9), "no_due".into(), priority(100), vec![], false, fixed_time(), None),
        ]
    }

    fn names(cards: &[Card]) -> Vec<&str> {
        cards.iter().map(|c| c.content()).collect()
    }

    #[test]
    fn due_filter_upcoming_all() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let mut cards = due_date_cards(today);
        apply_due_date_filter_with_today(&mut cards, DueDateFilter::Upcoming, today, false);
        let n = names(&cards);
        assert!(n.contains(&"tomorrow"));
        assert!(n.contains(&"in20days"));
        assert!(!n.contains(&"today"));
        assert!(!n.contains(&"yesterday"));
        assert!(!n.contains(&"no_due"));
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
        assert!(n.contains(&"tomorrow"));
        assert!(n.contains(&"in3days"));
        assert!(n.contains(&"in7days"));
        assert!(!n.contains(&"today"));
        assert!(!n.contains(&"in10days"));
        assert!(!n.contains(&"no_due"));
    }

    #[test]
    fn due_filter_upcoming_two_weeks() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let mut cards = due_date_cards(today);
        apply_due_date_filter_with_today(&mut cards, DueDateFilter::UpcomingTwoWeeks, today, false);
        let n = names(&cards);
        assert!(n.contains(&"tomorrow"));
        assert!(n.contains(&"in3days"));
        assert!(n.contains(&"in7days"));
        assert!(n.contains(&"in10days"));
        assert!(n.contains(&"in14days"));
        assert!(!n.contains(&"today"));
        assert!(!n.contains(&"in20days"));
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
        assert!(n.contains(&"tomorrow"));
        assert!(n.contains(&"in3days"));
        assert!(n.contains(&"in7days"));
        assert!(!n.contains(&"today"));
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
    fn due_filter_is_upcoming() {
        assert!(!DueDateFilter::All.is_upcoming());
        assert!(!DueDateFilter::Overdue.is_upcoming());
        assert!(!DueDateFilter::Today.is_upcoming());
        assert!(DueDateFilter::TodayAndUpcoming.is_upcoming());
        assert!(DueDateFilter::Upcoming.is_upcoming());
        assert!(DueDateFilter::UpcomingTomorrow.is_upcoming());
        assert!(DueDateFilter::UpcomingWeek.is_upcoming());
        assert!(DueDateFilter::UpcomingTwoWeeks.is_upcoming());
    }

    #[test]
    fn due_filter_labels_non_empty() {
        let variants = [
            DueDateFilter::All,
            DueDateFilter::Overdue,
            DueDateFilter::Today,
            DueDateFilter::TodayAndUpcoming,
            DueDateFilter::Upcoming,
            DueDateFilter::UpcomingTomorrow,
            DueDateFilter::UpcomingWeek,
            DueDateFilter::UpcomingTwoWeeks,
        ];
        for v in &variants {
            assert!(!v.label().is_empty(), "label for {:?} is empty", v);
        }
    }
}
