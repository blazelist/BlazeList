//! Markdown display utilities shared across BlazeList clients.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use blazelist_protocol::{Card, Entity};
use chrono::{DateTime, Utc};
use comrak::nodes::NodeValue;
use comrak::{Arena, Options, parse_document};
use uuid::Uuid;

/// Regex matching UUID-formatted strings (8-4-4-4-12 hex).
static UUID_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
    )
    .expect("UUID regex is valid")
});

/// Regex matching HTML tags (for splitting HTML into tags and text).
static HTML_TAG_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"<[^>]*>").expect("HTML tag regex is valid"));

/// Render a Markdown string as plain text by removing all formatting.
///
/// Uses `comrak` ([GitHub Flavored Markdown](https://github.github.com/gfm/))
/// to properly parse headings, bold, italic, strikethrough, inline code,
/// links, images, etc. and returns only the text content.
pub fn render_plain_text(line: &str) -> String {
    let arena = Arena::new();
    let root = parse_document(&arena, line, &markdown_options());

    let mut result = String::with_capacity(line.len());
    collect_text(root, &mut result);
    result
}

/// Generate a plain-text preview from card content for a card list.
///
/// Takes the first line, removes Markdown formatting, and truncates with `…`
/// if the result exceeds `max_width`. Returns `None` if content is empty.
pub fn card_preview(content: &str, max_width: usize) -> Option<String> {
    if max_width == 0 {
        return None;
    }
    let first_line = content.lines().next()?;
    if first_line.trim().is_empty() {
        return None;
    }
    let plain = render_plain_text(first_line);
    if plain.is_empty() {
        return None;
    }
    if plain.chars().count() > max_width {
        let truncated: String = plain.chars().take(max_width.saturating_sub(1)).collect();
        Some(format!("{truncated}…"))
    } else {
        Some(plain)
    }
}

/// Standard comrak options used across the project.
pub fn markdown_options() -> Options<'static> {
    let mut opts = Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.autolink = true;
    opts.extension.tasklist = true;
    opts
}

/// Toggle the `index`-th GFM task-list checkbox in a markdown string.
///
/// Scans for lines matching `- [ ]` / `- [x]` (also `*`/`+` bullets, case-insensitive `x`)
/// and flips the one at `index`. Returns `Some(new_content)` on success, `None` if
/// `index` is out of bounds.
pub fn toggle_task_item(content: &str, index: usize) -> Option<String> {
    let mut current = 0usize;
    let mut result = String::with_capacity(content.len());
    let mut found = false;

    for (i, line) in content.lines().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        if let Some((prefix_len, check_byte)) = parse_task_item(line) {
            if current == index {
                let toggled = if check_byte == b' ' { "x" } else { " " };
                // prefix_len points to the '[', so replace [.] (3 bytes)
                result.push_str(&line[..prefix_len]);
                result.push_str(&format!("[{toggled}]"));
                result.push_str(&line[prefix_len + 3..]);
                found = true;
            } else {
                result.push_str(line);
            }
            current += 1;
        } else {
            result.push_str(line);
        }
    }

    // Preserve trailing newline if the original had one
    if content.ends_with('\n') {
        result.push('\n');
    }

    if found { Some(result) } else { None }
}

/// Try to parse a GFM task-list item from a line.
///
/// Returns `Some((bracket_offset, check_byte))` where `bracket_offset` is
/// the byte offset of `[` and `check_byte` is the character inside (`b' '`,
/// `b'x'`, or `b'X'`). Returns `None` if the line isn't a task item.
fn parse_task_item(line: &str) -> Option<(usize, u8)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    // Skip leading whitespace
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    // Expect a list bullet: -, *, or +
    if i >= bytes.len() || !matches!(bytes[i], b'-' | b'*' | b'+') {
        return None;
    }
    i += 1;
    // Expect at least one space
    if i >= bytes.len() || bytes[i] != b' ' {
        return None;
    }
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    // Expect [<check>] where check is ' ', 'x', or 'X'
    if i + 2 >= bytes.len() || bytes[i] != b'[' {
        return None;
    }
    let check = bytes[i + 1];
    if !matches!(check, b' ' | b'x' | b'X') {
        return None;
    }
    if bytes[i + 2] != b']' {
        return None;
    }
    Some((i, check))
}

/// Count completed and total task-list items in a markdown string.
///
/// Returns `Some((done, total))` if there is at least one task item,
/// `None` otherwise.
pub fn task_progress(content: &str) -> Option<(usize, usize)> {
    let mut done = 0usize;
    let mut total = 0usize;
    for line in content.lines() {
        if let Some((_, check)) = parse_task_item(line) {
            total += 1;
            if check != b' ' {
                done += 1;
            }
        }
    }
    if total > 0 { Some((done, total)) } else { None }
}

/// Extract UUIDs referenced in card content as linked card IDs.
///
/// Scans the content for UUID-formatted strings (8-4-4-4-12 hex format) and
/// returns a deduplicated list of parsed UUIDs, excluding `own_id` (the
/// card's own UUID). This is purely a frontend feature — no server changes.
pub fn extract_card_links(content: &str, own_id: Uuid) -> Vec<Uuid> {
    let mut links = Vec::new();
    let mut seen = HashSet::new();
    for m in UUID_RE.find_iter(content) {
        if let Ok(uuid) = m.as_str().parse::<Uuid>()
            && uuid != own_id
            && seen.insert(uuid)
        {
            links.push(uuid);
        }
    }
    links
}

/// Resolve linked card UUIDs to `(uuid, preview)` pairs.
///
/// For each UUID in `linked_ids`, looks up the card in `all_cards` and
/// generates a plain-text preview (truncated to `max_width`). IDs that
/// don't match any card are silently skipped.
pub fn resolve_linked_cards(
    linked_ids: &[Uuid],
    all_cards: &[Card],
    max_width: usize,
) -> Vec<(Uuid, String)> {
    let card_map: HashMap<Uuid, &Card> = all_cards.iter().map(|c| (c.id(), c)).collect();
    linked_ids
        .iter()
        .filter_map(|lid| {
            card_map.get(lid).map(|c| {
                let preview =
                    card_preview(c.content(), max_width).unwrap_or_else(|| "(empty)".to_string());
                (*lid, preview)
            })
        })
        .collect()
}

/// Find cards that link **to** the given card (back-links).
///
/// Scans every card in `all_cards` for UUID references pointing at
/// `card_id`, returning the IDs of the linking cards. Combined with
/// `extract_card_links` this gives bidirectional link resolution.
pub fn extract_back_links(card_id: Uuid, all_cards: &[Card]) -> Vec<Uuid> {
    let needle = card_id.to_string();
    all_cards
        .iter()
        .filter(|c| c.id() != card_id && c.content().contains(&needle))
        .map(|c| c.id())
        .collect()
}

/// Forward and backward link counts for a card.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkCounts {
    /// Number of other cards this card's content references.
    pub forward: usize,
    /// Number of other cards whose content references this card.
    pub back: usize,
}

/// Compute link counts for every card in a single pass.
///
/// For each card, extracts forward links (UUIDs in content) and increments
/// the back-link count on the referenced cards. Returns a map from card ID
/// to its [`LinkCounts`].
pub fn compute_all_link_counts(cards: &[Card]) -> HashMap<Uuid, LinkCounts> {
    let card_ids: HashSet<Uuid> = cards.iter().map(|c| c.id()).collect();
    let mut counts: HashMap<Uuid, LinkCounts> = HashMap::new();

    for card in cards {
        let own_id = card.id();
        let forward = extract_card_links(card.content(), own_id);

        let valid: Vec<Uuid> = forward
            .into_iter()
            .filter(|id| card_ids.contains(id))
            .collect();
        counts.entry(own_id).or_default().forward = valid.len();

        for target_id in valid {
            counts.entry(target_id).or_default().back += 1;
        }
    }

    counts
}

/// Post-process rendered HTML to wrap known card UUIDs in clickable spans.
///
/// Only replaces UUIDs found in text content (outside HTML tags) to avoid
/// corrupting attributes. Each matched UUID is wrapped in a
/// `<span class="card-uuid-link" data-card-id="UUID">` element.
pub fn linkify_card_uuids(html: &str, card_ids: &HashSet<Uuid>) -> String {
    if card_ids.is_empty() {
        return html.to_string();
    }

    let mut result = String::with_capacity(html.len() + html.len() / 10);
    let mut last_end = 0;

    for tag_match in HTML_TAG_RE.find_iter(html) {
        // Process text before this tag — UUIDs here are safe to wrap.
        linkify_segment(&html[last_end..tag_match.start()], card_ids, &mut result);
        // Append the tag as-is (don't touch attributes).
        result.push_str(tag_match.as_str());
        last_end = tag_match.end();
    }
    // Remaining text after the last tag.
    linkify_segment(&html[last_end..], card_ids, &mut result);

    result
}

/// Replace UUIDs in a text segment (outside HTML tags) with clickable spans.
fn linkify_segment(text: &str, card_ids: &HashSet<Uuid>, out: &mut String) {
    let mut last = 0;
    for m in UUID_RE.find_iter(text) {
        out.push_str(&text[last..m.start()]);
        if let Ok(uuid) = m.as_str().parse::<Uuid>() {
            if card_ids.contains(&uuid) {
                out.push_str(r#"<span class="card-uuid-link" data-card-id=""#);
                out.push_str(&uuid.to_string());
                out.push_str(r#"">"#);
                out.push_str(m.as_str());
                out.push_str("</span>");
            } else {
                out.push_str(m.as_str());
            }
        } else {
            out.push_str(m.as_str());
        }
        last = m.end();
    }
    out.push_str(&text[last..]);
}

/// Format a timestamp as a human-readable relative time string.
pub fn format_relative_time(ts: &DateTime<Utc>) -> String {
    let elapsed = Utc::now().signed_duration_since(ts);
    let secs = elapsed.num_seconds();
    if secs < 5 {
        "just now".into()
    } else if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        let mins = secs / 60;
        format!("{mins}m ago")
    } else if secs < 86400 {
        let hours = secs / 3600;
        format!("{hours}h ago")
    } else {
        let days = secs / 86400;
        format!("{days}d ago")
    }
}

/// Recursively collect text content from a comrak AST node.
pub fn collect_text<'a>(node: &'a comrak::nodes::AstNode<'a>, out: &mut String) {
    match &node.data.borrow().value {
        NodeValue::Text(text) => out.push_str(text),
        NodeValue::Code(code) => out.push_str(&code.literal),
        NodeValue::SoftBreak | NodeValue::LineBreak => {}
        _ => {
            for child in node.children() {
                collect_text(child, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    #[test]
    fn render_plain_text_headings() {
        assert_eq!(render_plain_text("# Hello"), "Hello");
        assert_eq!(render_plain_text("## World"), "World");
        assert_eq!(render_plain_text("### Foo Bar"), "Foo Bar");
    }

    #[test]
    fn render_plain_text_formatting() {
        assert_eq!(render_plain_text("**bold** text"), "bold text");
        assert_eq!(render_plain_text("*italic* word"), "italic word");
        assert_eq!(render_plain_text("`code` here"), "code here");
        assert_eq!(render_plain_text("~~strike~~"), "strike");
    }

    #[test]
    fn render_plain_text_links() {
        assert_eq!(render_plain_text("[click](http://example.com)"), "click");
        assert_eq!(
            render_plain_text("See [link](http://example.com) here"),
            "See link here"
        );
    }

    #[test]
    fn render_plain_text_plain() {
        assert_eq!(render_plain_text("just plain text"), "just plain text");
    }

    #[test]
    fn render_plain_text_empty() {
        assert_eq!(render_plain_text(""), "");
    }

    #[test]
    fn card_preview_first_line_only() {
        assert_eq!(
            card_preview("# Title\nBody text here", 100),
            Some("Title".to_string())
        );
    }

    #[test]
    fn card_preview_truncation() {
        assert_eq!(card_preview("Hello World", 5), Some("Hell…".to_string()));
    }

    #[test]
    fn card_preview_exact_fit() {
        assert_eq!(card_preview("Hello", 5), Some("Hello".to_string()));
    }

    #[test]
    fn card_preview_empty() {
        assert_eq!(card_preview("", 100), None);
        assert_eq!(card_preview("   ", 100), None);
    }

    #[test]
    fn card_preview_zero_width() {
        assert_eq!(card_preview("Hello", 0), None);
    }

    #[test]
    fn card_preview_strips_markdown() {
        assert_eq!(
            card_preview("**bold title**", 100),
            Some("bold title".to_string())
        );
    }

    #[test]
    fn toggle_task_item_check() {
        let md = "- [ ] Buy tofu\n- [ ] Walk dog";
        let result = toggle_task_item(md, 0).unwrap();
        assert_eq!(result, "- [x] Buy tofu\n- [ ] Walk dog");
    }

    #[test]
    fn toggle_task_item_uncheck() {
        let md = "- [x] Buy tofu\n- [ ] Walk dog";
        let result = toggle_task_item(md, 0).unwrap();
        assert_eq!(result, "- [ ] Buy tofu\n- [ ] Walk dog");
    }

    #[test]
    fn toggle_task_item_second() {
        let md = "- [ ] Buy tofu\n- [ ] Walk dog";
        let result = toggle_task_item(md, 1).unwrap();
        assert_eq!(result, "- [ ] Buy tofu\n- [x] Walk dog");
    }

    #[test]
    fn toggle_task_item_out_of_bounds() {
        let md = "- [ ] Buy tofu";
        assert!(toggle_task_item(md, 1).is_none());
    }

    #[test]
    fn toggle_task_item_mixed_content() {
        let md = "# Title\n- [ ] First\nSome text\n- [x] Second\n- [ ] Third";
        let result = toggle_task_item(md, 1).unwrap();
        assert_eq!(
            result,
            "# Title\n- [ ] First\nSome text\n- [ ] Second\n- [ ] Third"
        );
    }

    #[test]
    fn toggle_task_item_preserves_trailing_newline() {
        let md = "- [ ] Item\n";
        let result = toggle_task_item(md, 0).unwrap();
        assert_eq!(result, "- [x] Item\n");
    }

    #[test]
    fn toggle_task_item_star_bullet() {
        let md = "* [ ] Star item";
        let result = toggle_task_item(md, 0).unwrap();
        assert_eq!(result, "* [x] Star item");
    }

    #[test]
    fn toggle_task_item_uppercase_x() {
        let md = "- [X] Done item";
        let result = toggle_task_item(md, 0).unwrap();
        assert_eq!(result, "- [ ] Done item");
    }

    #[test]
    fn markdown_options_enables_extensions() {
        let opts = markdown_options();
        assert!(opts.extension.strikethrough);
        assert!(opts.extension.table);
        assert!(opts.extension.autolink);
        assert!(opts.extension.tasklist);
    }

    #[test]
    fn extract_card_links_finds_uuids() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let linked = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let content = format!("See card {linked} for details");
        let result = extract_card_links(&content, own);
        assert_eq!(result, vec![linked]);
    }

    #[test]
    fn extract_card_links_excludes_own_id() {
        let own = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let content = format!("Self-reference: {own}");
        let result = extract_card_links(&content, own);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_card_links_deduplicates() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let linked = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let content = format!("First: {linked}\nSecond: {linked}");
        let result = extract_card_links(&content, own);
        assert_eq!(result, vec![linked]);
    }

    #[test]
    fn extract_card_links_multiple() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let a = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let b = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        let content = format!("Links: {a} and {b}");
        let result = extract_card_links(&content, own);
        assert_eq!(result, vec![a, b]);
    }

    #[test]
    fn extract_card_links_empty_content() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let result = extract_card_links("", own);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_card_links_no_uuids() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let result = extract_card_links("Just some regular text", own);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_card_links_uuid_at_start() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let linked = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let content = format!("{linked} is referenced");
        let result = extract_card_links(&content, own);
        assert_eq!(result, vec![linked]);
    }

    #[test]
    fn extract_card_links_uuid_at_end() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let linked = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let content = format!("See {linked}");
        let result = extract_card_links(&content, own);
        assert_eq!(result, vec![linked]);
    }

    #[test]
    fn extract_card_links_uuid_only() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let linked = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let result = extract_card_links(&linked.to_string(), own);
        assert_eq!(result, vec![linked]);
    }

    #[test]
    fn extract_card_links_content_shorter_than_uuid() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let result = extract_card_links("short", own);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_card_links_almost_uuid() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        // Valid format but with non-hex characters
        let result = extract_card_links("zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz", own);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_card_links_hyphens_wrong_positions() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let result = extract_card_links("0000000-00000-0000-0000-000000000002", own);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_card_links_uuid_in_markdown() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let a = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        let b = Uuid::parse_str("66666666-7777-8888-9999-aaaaaaaaaaaa").unwrap();
        let content =
            format!("# Task list\n\n- Follow up on {a}\n- Also see {b}\n- Own ref {own} ignored");
        let result = extract_card_links(&content, own);
        assert_eq!(result, vec![a, b]);
    }

    #[test]
    fn extract_card_links_back_to_back() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let a = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let b = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        // Two UUIDs with no separator
        let content = format!("{a}{b}");
        let result = extract_card_links(&content, own);
        assert_eq!(result, vec![a, b]);
    }

    #[test]
    fn extract_card_links_preserves_order() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let c = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap();
        let a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        let content = format!("{c} then {a} then {b}");
        let result = extract_card_links(&content, own);
        // Order should match appearance in content, not sorted
        assert_eq!(result, vec![c, a, b]);
    }

    #[test]
    fn extract_card_links_mixed_own_and_others() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        let content = format!("{a} {own} {b} {own} {a}");
        let result = extract_card_links(&content, own);
        // own excluded, a appears twice but deduplicated
        assert_eq!(result, vec![a, b]);
    }

    #[test]
    fn extract_card_links_in_url() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let linked = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let content = format!("https://example.com/card/{linked}");
        let result = extract_card_links(&content, own);
        assert_eq!(result, vec![linked]);
    }

    #[test]
    fn extract_card_links_in_markdown_link() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let linked = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let content = format!("[linked card]({linked})");
        let result = extract_card_links(&content, own);
        assert_eq!(result, vec![linked]);
    }

    #[test]
    fn extract_card_links_uppercase_hex() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let result = extract_card_links("AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE", own);
        let expected = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        assert_eq!(result, vec![expected]);
    }

    #[test]
    fn extract_card_links_mixed_case_hex() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let result = extract_card_links("AaAaAaAa-BbBb-CcCc-DdDd-EeEeEeEeEeEe", own);
        let expected = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        assert_eq!(result, vec![expected]);
    }

    /// Expect test: comprehensive snapshot of extract_card_links behavior.
    #[test]
    fn extract_card_links_expect_basic() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let content = "See card aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee for details";
        let result = extract_card_links(content, own);
        let formatted: Vec<String> = result.iter().map(|u| u.to_string()).collect();
        expect![[r#"
            [
                "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            ]
        "#]]
        .assert_debug_eq(&formatted);
    }

    #[test]
    fn extract_card_links_expect_complex_markdown() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let content = "\
# Shopping list

Related to 11111111-2222-3333-4444-555555555555

## Notes
- See also 66666666-7777-8888-9999-aaaaaaaaaaaa
- Self ref 00000000-0000-0000-0000-000000000001 should be excluded
- Duplicate 11111111-2222-3333-4444-555555555555 should be deduplicated

Done.";
        let result = extract_card_links(content, own);
        let formatted: Vec<String> = result.iter().map(|u| u.to_string()).collect();
        expect![[r#"
            [
                "11111111-2222-3333-4444-555555555555",
                "66666666-7777-8888-9999-aaaaaaaaaaaa",
            ]
        "#]]
        .assert_debug_eq(&formatted);
    }

    #[test]
    fn extract_card_links_expect_edge_cases() {
        let own = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

        // Empty
        let r1 = extract_card_links("", own);

        // Too short
        let r2 = extract_card_links("abcdefgh-1234", own);

        // Only own
        let r3 = extract_card_links("00000000-0000-0000-0000-000000000001", own);

        // Invalid hex chars in UUID position
        let r4 = extract_card_links("zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz", own);

        // Back-to-back UUIDs
        let r5 = extract_card_links(
            "11111111-1111-1111-1111-11111111111122222222-2222-2222-2222-222222222222",
            own,
        );

        let formatted = (
            r1.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
            r2.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
            r3.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
            r4.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
            r5.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
        );
        expect![[r#"
            (
                [],
                [],
                [],
                [],
                [
                    "11111111-1111-1111-1111-111111111111",
                    "22222222-2222-2222-2222-222222222222",
                ],
            )
        "#]]
        .assert_debug_eq(&formatted);
    }

    #[test]
    fn extract_card_links_expect_multiline_document() {
        let own = Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap();
        let content = "\
> **Conflict** — 2025-01-01, from client-1
> This card was created from a conflict with card aabbccdd-1122-3344-5566-778899001122.
> The server version of that card was kept as-is.

Original content here with another ref: 12345678-abcd-ef01-2345-6789abcdef01

And own ref ffffffff-ffff-ffff-ffff-ffffffffffff excluded.";
        let result = extract_card_links(content, own);
        let formatted: Vec<String> = result.iter().map(|u| u.to_string()).collect();
        expect![[r#"
            [
                "aabbccdd-1122-3344-5566-778899001122",
                "12345678-abcd-ef01-2345-6789abcdef01",
            ]
        "#]]
        .assert_debug_eq(&formatted);
    }

    #[test]
    fn resolve_linked_cards_basic() {
        use blazelist_protocol::NonNegativeI64;
        use chrono::DateTime;

        let t = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let p = NonNegativeI64::try_from(1000i64).unwrap();
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        let id_c = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap();

        let cards = vec![
            Card::first(id_a, "Alpha card".into(), p, vec![], false, t, None),
            Card::first(id_b, "**Beta** card".into(), p, vec![], false, t, None),
        ];

        // Resolves matching cards with previews, skips unknown id_c
        let result = resolve_linked_cards(&[id_a, id_c, id_b], &cards, 40);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], (id_a, "Alpha card".to_string()));
        assert_eq!(result[1], (id_b, "Beta card".to_string()));
    }

    #[test]
    fn resolve_linked_cards_empty_content() {
        use blazelist_protocol::NonNegativeI64;
        use chrono::DateTime;

        let t = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let p = NonNegativeI64::try_from(1000i64).unwrap();
        let id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();

        let cards = vec![Card::first(id, "".into(), p, vec![], false, t, None)];
        let result = resolve_linked_cards(&[id], &cards, 40);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], (id, "(empty)".to_string()));
    }

    #[test]
    fn resolve_linked_cards_empty_ids() {
        let result = resolve_linked_cards(&[], &[], 40);
        assert!(result.is_empty());
    }

    #[test]
    fn resolve_linked_cards_truncates_preview() {
        use blazelist_protocol::NonNegativeI64;
        use chrono::DateTime;

        let t = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let p = NonNegativeI64::try_from(1000i64).unwrap();
        let id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();

        let cards = vec![Card::first(
            id,
            "This is a very long card title that should get truncated".into(),
            p,
            vec![],
            false,
            t,
            None,
        )];
        let result = resolve_linked_cards(&[id], &cards, 10);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1, "This is a…");
    }

    #[test]
    fn extract_back_links_finds_referencing_cards() {
        use blazelist_protocol::NonNegativeI64;
        use chrono::DateTime;

        let t = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let p = NonNegativeI64::try_from(1000i64).unwrap();
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        let id_c = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap();

        let cards = vec![
            Card::first(id_a, "Standalone".into(), p, vec![], false, t, None),
            Card::first(id_b, format!("Links to {id_a}"), p, vec![], false, t, None),
            Card::first(
                id_c,
                format!("Also links to {id_a}"),
                p,
                vec![],
                false,
                t,
                None,
            ),
        ];

        let back = extract_back_links(id_a, &cards);
        assert_eq!(back, vec![id_b, id_c]);
    }

    #[test]
    fn extract_back_links_excludes_self() {
        use blazelist_protocol::NonNegativeI64;
        use chrono::DateTime;

        let t = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let p = NonNegativeI64::try_from(1000i64).unwrap();
        let id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();

        // Card references its own UUID — should not appear as a back-link.
        let cards = vec![Card::first(
            id,
            format!("Self ref {id}"),
            p,
            vec![],
            false,
            t,
            None,
        )];
        let back = extract_back_links(id, &cards);
        assert!(back.is_empty());
    }

    #[test]
    fn extract_back_links_empty_when_no_references() {
        use blazelist_protocol::NonNegativeI64;
        use chrono::DateTime;

        let t = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let p = NonNegativeI64::try_from(1000i64).unwrap();
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();

        let cards = vec![
            Card::first(id_a, "No refs".into(), p, vec![], false, t, None),
            Card::first(id_b, "Also no refs".into(), p, vec![], false, t, None),
        ];

        let back = extract_back_links(id_a, &cards);
        assert!(back.is_empty());
    }

    // ── compute_all_link_counts ──────────────────────────────────────

    #[test]
    fn compute_all_link_counts_basic() {
        use blazelist_protocol::NonNegativeI64;
        use chrono::DateTime;

        let t = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let p = NonNegativeI64::try_from(1000i64).unwrap();
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        let id_c = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap();

        // A references B and C, B references C
        let cards = vec![
            Card::first(
                id_a,
                format!("Links to {id_b} and {id_c}"),
                p,
                vec![],
                false,
                t,
                None,
            ),
            Card::first(id_b, format!("Links to {id_c}"), p, vec![], false, t, None),
            Card::first(id_c, "No links".into(), p, vec![], false, t, None),
        ];

        let counts = compute_all_link_counts(&cards);

        // A: 2 forward, 0 back
        assert_eq!(counts[&id_a].forward, 2);
        assert_eq!(counts[&id_a].back, 0);
        // B: 1 forward, 1 back (from A)
        assert_eq!(counts[&id_b].forward, 1);
        assert_eq!(counts[&id_b].back, 1);
        // C: 0 forward, 2 back (from A and B)
        assert_eq!(counts[&id_c].forward, 0);
        assert_eq!(counts[&id_c].back, 2);
    }

    #[test]
    fn compute_all_link_counts_self_ref_excluded() {
        use blazelist_protocol::NonNegativeI64;
        use chrono::DateTime;

        let t = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let p = NonNegativeI64::try_from(1000i64).unwrap();
        let id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();

        let cards = vec![Card::first(
            id,
            format!("Self ref {id}"),
            p,
            vec![],
            false,
            t,
            None,
        )];

        let counts = compute_all_link_counts(&cards);
        assert_eq!(counts.get(&id).map(|c| c.forward).unwrap_or(0), 0);
        assert_eq!(counts.get(&id).map(|c| c.back).unwrap_or(0), 0);
    }

    #[test]
    fn compute_all_link_counts_duplicate_uuid_counted_once() {
        use blazelist_protocol::NonNegativeI64;
        use chrono::DateTime;

        let t = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let p = NonNegativeI64::try_from(1000i64).unwrap();
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();

        // A mentions B twice
        let cards = vec![
            Card::first(
                id_a,
                format!("{id_b} again {id_b}"),
                p,
                vec![],
                false,
                t,
                None,
            ),
            Card::first(id_b, "target".into(), p, vec![], false, t, None),
        ];

        let counts = compute_all_link_counts(&cards);
        assert_eq!(counts[&id_a].forward, 1);
        assert_eq!(counts[&id_b].back, 1);
    }

    #[test]
    fn compute_all_link_counts_nonexistent_target_ignored() {
        use blazelist_protocol::NonNegativeI64;
        use chrono::DateTime;

        let t = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let p = NonNegativeI64::try_from(1000i64).unwrap();
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let ghost = Uuid::parse_str("dddddddd-dddd-dddd-dddd-dddddddddddd").unwrap();

        // A references a UUID that is not in the card set
        let cards = vec![Card::first(
            id_a,
            format!("ref {ghost}"),
            p,
            vec![],
            false,
            t,
            None,
        )];

        let counts = compute_all_link_counts(&cards);
        assert_eq!(counts.get(&id_a).map(|c| c.forward).unwrap_or(0), 0);
    }

    // ── linkify_card_uuids ──────────────────────────────────────────

    #[test]
    fn linkify_card_uuids_wraps_known_ids() {
        let id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let card_ids: std::collections::HashSet<Uuid> = [id].into_iter().collect();

        let html = format!("<p>See {id} for details</p>");
        let result = linkify_card_uuids(&html, &card_ids);

        assert!(result.contains("class=\"card-uuid-link\""));
        assert!(result.contains(&format!("data-card-id=\"{id}\"")));
        // Original text preserved
        assert!(result.contains("See "));
        assert!(result.contains(" for details"));
    }

    #[test]
    fn linkify_card_uuids_ignores_unknown_ids() {
        let known = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let unknown = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        let card_ids: std::collections::HashSet<Uuid> = [known].into_iter().collect();

        let html = format!("<p>{unknown}</p>");
        let result = linkify_card_uuids(&html, &card_ids);

        // Should be unchanged
        assert_eq!(result, html);
    }

    #[test]
    fn linkify_card_uuids_does_not_modify_html_attributes() {
        let id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let card_ids: std::collections::HashSet<Uuid> = [id].into_iter().collect();

        // UUID inside an href attribute should NOT be linkified
        let html = format!(r#"<a href="https://example.com/{id}">link</a>"#);
        let result = linkify_card_uuids(&html, &card_ids);

        // The href should be intact
        assert!(result.contains(&format!("href=\"https://example.com/{id}\"")));
        // "link" text has no UUID so no wrapping there
        assert!(!result.contains("card-uuid-link"));
    }

    #[test]
    fn linkify_card_uuids_empty_set_noop() {
        let card_ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        let html = "<p>aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa</p>";
        let result = linkify_card_uuids(html, &card_ids);
        assert_eq!(result, html);
    }

    #[test]
    fn linkify_card_uuids_multiple_ids_in_one_paragraph() {
        let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        let card_ids: std::collections::HashSet<Uuid> = [id_a, id_b].into_iter().collect();

        let html = format!("<p>{id_a} and {id_b}</p>");
        let result = linkify_card_uuids(&html, &card_ids);

        // Both should be wrapped
        assert_eq!(result.matches("card-uuid-link").count(), 2);
        assert!(result.contains(&format!("data-card-id=\"{id_a}\"")));
        assert!(result.contains(&format!("data-card-id=\"{id_b}\"")));
    }
}
