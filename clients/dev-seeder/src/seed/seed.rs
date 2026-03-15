//! Deterministic seed data generation.
//!
//! Content, structure, and relative ordering are driven by a seeded
//! [`ChaCha8Rng`] for reproducibility. Timestamps are anchored to the current
//! wall-clock time so seeded data looks recent.

use std::collections::BTreeSet;

use blazelist_protocol::{Card, Entity, PushItem, Tag};
use chrono::{DateTime, Duration, Utc};
use fake::Fake;
use fake::faker::lorem::en::*;
use rand::RngCore;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rgb::RGB8;
use uuid::Uuid;

/// Seed data ready for pushing to a BlazeList server.
pub struct SeedData {
    /// Live tag version chains.
    pub tag_chains: Vec<Vec<Tag>>,
    /// Live card version chains.
    pub card_chains: Vec<Vec<Card>>,
    /// Tag chains that will be created then deleted.
    pub deleted_tag_chains: Vec<Vec<Tag>>,
    /// Card chains that will be created then deleted.
    pub deleted_card_chains: Vec<Vec<Card>>,
    /// Extra operations pushed individually (one sequence entry each).
    pub extra_ops: Vec<Vec<PushItem>>,
}

/// Pre-defined tag colors for visual variety.
const TAG_COLORS: &[RGB8] = &[
    RGB8::new(0xc0, 0x40, 0x40), // red
    RGB8::new(0xc0, 0x78, 0x30), // amber/orange
    RGB8::new(0x48, 0xa8, 0x5a), // green
    RGB8::new(0x3c, 0x8c, 0xc4), // blue
    RGB8::new(0x90, 0x60, 0xc0), // purple
    RGB8::new(0xc0, 0x60, 0x80), // pink
    RGB8::new(0x40, 0xa0, 0xa0), // teal
    RGB8::new(0xd4, 0xa0, 0x20), // gold
    RGB8::new(0x60, 0x80, 0xc0), // slate blue
    RGB8::new(0xa0, 0x60, 0x30), // brown
];

/// Pick a random color from the palette (~60% chance) or None.
fn random_tag_color(rng: &mut ChaCha8Rng) -> Option<RGB8> {
    if rng.next_u32() % 100 < 60 {
        let idx = rng.next_u32() as usize % TAG_COLORS.len();
        Some(TAG_COLORS[idx])
    } else {
        None
    }
}

/// Types of edits that can be applied to a card version.
#[derive(Clone, Copy)]
enum CardEdit {
    Content,
    Priority,
    Tags,
    Blazed,
    DueDate,
}

const ALL_CARD_EDITS: [CardEdit; 5] = [
    CardEdit::Content,
    CardEdit::Priority,
    CardEdit::Tags,
    CardEdit::Blazed,
    CardEdit::DueDate,
];

/// Generate a deterministic UUIDv4 from the given RNG.
fn gen_uuid(rng: &mut ChaCha8Rng) -> Uuid {
    let mut bytes = [0u8; 16];
    rng.fill_bytes(&mut bytes);
    // Set version 4 and variant 1 bits for a valid UUIDv4.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Generate a deterministic timestamp up to ~1 year before `base`.
fn gen_timestamp(rng: &mut ChaCha8Rng, base: DateTime<Utc>) -> DateTime<Utc> {
    let offset_secs = (rng.next_u64() % (365 * 24 * 3600)) as i64;
    base - Duration::seconds(offset_secs)
}

/// Generate an optional due date biased toward `now`.
///
/// Distribution:
/// - ~35%: no due date
/// - ~25%: near today (±7 days)
/// - ~20%: medium range (±30 days)
/// - ~12%: wider (±90 days)
/// - ~8%:  far out (±180 days)
fn gen_due_date(rng: &mut ChaCha8Rng, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let roll = rng.next_u32() % 100;
    if roll < 35 {
        return None;
    }

    let offset_days = if roll < 60 {
        (rng.next_u32() % 15) as i64 - 7
    } else if roll < 80 {
        (rng.next_u32() % 61) as i64 - 30
    } else if roll < 92 {
        (rng.next_u32() % 181) as i64 - 90
    } else {
        (rng.next_u32() % 361) as i64 - 180
    };

    Some(now + Duration::days(offset_days))
}

/// Upper bound for priority space (= `i64::MAX`).
const MAX_PRIORITY: i64 = i64::MAX;

/// Deterministically resolve a priority collision.
///
/// If `priority` is not already in `used`, returns it unchanged.
/// Otherwise, finds the next free priority by computing the midpoint of
/// the largest adjacent gap — inspired by the client lib's placement
/// logic, but fully deterministic (no RNG / jitter).
pub(super) fn resolve_priority(priority: i64, used: &BTreeSet<i64>) -> i64 {
    if !used.contains(&priority) {
        return priority;
    }

    // Find the nearest used priority above and below.
    let upper = used
        .range((priority + 1)..)
        .next()
        .copied()
        .unwrap_or(MAX_PRIORITY);
    let lower = used.range(..priority).next_back().copied().unwrap_or(i64::MIN);

    let gap_above = upper as i128 - priority as i128;
    let gap_below = priority as i128 - lower as i128;

    // Pick the gap with more room and use its midpoint.
    if gap_above >= gap_below && gap_above > 1 {
        (priority as i128 + gap_above / 2) as i64
    } else if gap_below > 1 {
        (lower as i128 + gap_below / 2) as i64
    } else if gap_above > 1 {
        (priority as i128 + gap_above / 2) as i64
    } else {
        // Both immediate gaps are exhausted — scan for the largest gap in
        // the entire set (extremely unlikely in practice).
        find_largest_gap_midpoint(used)
    }
}

/// Scan the full `used` set and return the midpoint of the largest gap.
fn find_largest_gap_midpoint(used: &BTreeSet<i64>) -> i64 {
    let mut best_gap_midpoint: i64 = MAX_PRIORITY / 2;
    let mut best_gap: i128 = 0;

    let mut prev = i64::MIN;
    for &p in used {
        let gap = p as i128 - prev as i128;
        if gap > best_gap {
            best_gap = gap;
            best_gap_midpoint = (prev as i128 + gap / 2) as i64;
        }
        prev = p;
    }
    // Check the gap after the last element.
    let gap = MAX_PRIORITY as i128 - prev as i128;
    if gap > best_gap {
        best_gap_midpoint = (prev as i128 + gap / 2) as i64;
    }

    best_gap_midpoint
}

/// Generate `num_tags` tag version chains deterministically.
fn generate_tags(rng: &mut ChaCha8Rng, num_tags: usize, base_time: DateTime<Utc>) -> Vec<Vec<Tag>> {
    (0..num_tags)
        .map(|_| {
            let id = gen_uuid(rng);
            let words: Vec<String> = Words(1..3).fake_with_rng(rng);
            let title = capitalize_first(&words.join(" "));
            let color = random_tag_color(rng);
            let created_at = gen_timestamp(rng, base_time);
            let first = Tag::first(id, title, color, created_at);
            generate_tag_history(rng, first, base_time)
        })
        .collect()
}

/// Generate version history for a tag (~30% get 1-3 renames).
///
/// Edits are spread between the tag's creation time and `now`.
fn generate_tag_history(rng: &mut ChaCha8Rng, first: Tag, now: DateTime<Utc>) -> Vec<Tag> {
    let roll = rng.next_u32() % 100;

    // ~30% of tags get rename history.
    if roll >= 30 {
        return vec![first];
    }

    let num_edits = 1 + (rng.next_u32() as usize % 3); // 1-3 renames
    let edit_times = spread_timestamps(rng, first.created_at(), now, num_edits);

    let mut chain = Vec::with_capacity(1 + num_edits);
    chain.push(first);

    for &modified_at in &edit_times {
        let prev = chain.last().unwrap();
        let new_title = mutate_tag_title(rng, prev.title());
        chain.push(prev.next(new_title, prev.color(), modified_at));
    }

    chain
}

/// Mutate a tag title for version history.
fn mutate_tag_title(rng: &mut ChaCha8Rng, current: &str) -> String {
    const SUFFIXES: &[&str] = &["List", "Tasks", "Items", "Tracker", "Log"];
    match rng.next_u32() % 3 {
        0 => {
            let suffix = SUFFIXES[rng.next_u32() as usize % SUFFIXES.len()];
            format!("{current} {suffix}")
        }
        1 => format!("My {current}"),
        _ => {
            let words: Vec<String> = Words(1..3).fake_with_rng(rng);
            capitalize_first(&words.join(" "))
        }
    }
}

/// Generate `num_cards` card version chains deterministically, referencing the
/// given tag IDs.
fn generate_cards(
    rng: &mut ChaCha8Rng,
    num_cards: usize,
    tag_ids: &[Uuid],
    base_time: DateTime<Utc>,
    used_priorities: &mut BTreeSet<i64>,
) -> Vec<Vec<Card>> {
    // Space priorities evenly across the full i64 range (i64::MIN..=i64::MAX).
    // Use i128 arithmetic to avoid overflow.
    let range: i128 = i64::MAX as i128 - i64::MIN as i128;
    let step = range / (num_cards as i128 + 1);

    (0..num_cards)
        .map(|i| {
            let id = gen_uuid(rng);
            let content = gen_card_content(rng, i);
            let raw_priority = (i64::MAX as i128 - step * (i as i128 + 1)) as i64;
            let resolved = resolve_priority(raw_priority, used_priorities);
            used_priorities.insert(resolved);
            let priority = resolved;

            // Tag assignment distribution:
            //   ~20% of cards: no tags
            //   ~40% of cards: 1-2 tags
            //   ~30% of cards: 3-5 tags
            //   ~10% of cards: many tags (at least half of available tags)
            let tags = if tag_ids.is_empty() {
                vec![]
            } else {
                let available = tag_ids.len();
                let roll = rng.next_u32() % 100;
                let num_tags = if roll < 20 {
                    0
                } else if roll < 60 {
                    // 1-2 tags (clamped to available).
                    1 + (rng.next_u32() as usize % 2.min(available))
                } else if roll < 90 {
                    // 3-5 tags (clamped to available).
                    let lo = 3.min(available);
                    let hi = 5.min(available);
                    lo + (rng.next_u32() as usize % (hi - lo + 1))
                } else {
                    // Many tags: half to all available tags.
                    let lo = available.div_ceil(2);
                    lo + (rng.next_u32() as usize % (available - lo + 1))
                };
                pick_tags(rng, tag_ids, num_tags)
            };

            // ~40% of cards are blazed (~400 of 1000).
            let blazed = rng.next_u32() % 100 < 40;

            let created_at = gen_timestamp(rng, base_time);
            let due_date = gen_due_date(rng, base_time);

            let first = Card::first(id, content, priority, tags, blazed, created_at, due_date);
            generate_card_history(rng, first, tag_ids, base_time, used_priorities)
        })
        .collect()
}

/// Generate version history for a single card.
///
/// Distribution:
/// - 10% of cards: no history (1 version)
/// - 27% of cards: 1-2 edits (short history)
/// - 63% of cards: 3-7 edits (deeper history)
///
/// Edits are spread between the card's creation time and `now`.
fn generate_card_history(
    rng: &mut ChaCha8Rng,
    first: Card,
    tag_ids: &[Uuid],
    now: DateTime<Utc>,
    used_priorities: &mut BTreeSet<i64>,
) -> Vec<Card> {
    let roll = rng.next_u32() % 100;

    // 10% — no history beyond the first version.
    if roll < 10 {
        return vec![first];
    }

    // Of the 90% with history, 70% get 3+ edits (63% overall).
    let num_edits = if roll < 37 {
        1 + (rng.next_u32() as usize % 2) // 1-2 edits
    } else {
        3 + (rng.next_u32() as usize % 5) // 3-7 edits
    };

    let edit_times = spread_timestamps(rng, first.created_at(), now, num_edits);

    let mut chain = Vec::with_capacity(1 + num_edits);
    chain.push(first);

    for &modified_at in &edit_times {
        let prev = chain.last().unwrap();

        let mut content = prev.content().to_string();
        let mut priority = prev.priority();
        let mut tags = prev.tags().to_vec();
        let mut blazed = prev.blazed();
        let mut due_date = prev.due_date();

        // Pick 1-3 fields to change.
        let num_changes = 1 + (rng.next_u32() as usize % 3);
        let edits = pick_card_edits(rng, num_changes);

        for edit in &edits {
            match edit {
                CardEdit::Content => match rng.next_u32() % 3 {
                    // Full rewrite with a different pattern.
                    0 => {
                        let idx = rng.next_u32() as usize;
                        content = gen_card_content(rng, idx);
                    }
                    // Append an update note.
                    1 => {
                        let note: String = Sentence(6..14).fake_with_rng(rng);
                        content = format!("{content}\n\n**Update:** {note}");
                    }
                    // Prepend a note.
                    _ => {
                        let note: String = Sentence(4..10).fake_with_rng(rng);
                        content = format!("**Note:** {note}\n\n{content}");
                    }
                },
                CardEdit::Priority => {
                    // Small shift (±500K) — negligible vs the inter-card step.
                    let shift = (rng.next_u64() % 1_000_000) as i64 - 500_000;
                    let raw = priority.saturating_add(shift);
                    // Remove the old priority and resolve the new one.
                    used_priorities.remove(&priority);
                    let resolved = resolve_priority(raw, used_priorities);
                    used_priorities.insert(resolved);
                    priority = resolved;
                }
                CardEdit::Tags => {
                    if !tag_ids.is_empty() {
                        match rng.next_u32() % 3 {
                            // Remove a random tag.
                            0 if !tags.is_empty() => {
                                let idx = rng.next_u32() as usize % tags.len();
                                tags.remove(idx);
                            }
                            // Add a random tag (if not already present).
                            1 => {
                                let new_tag = tag_ids[rng.next_u32() as usize % tag_ids.len()];
                                if !tags.contains(&new_tag) {
                                    tags.push(new_tag);
                                }
                            }
                            // Re-pick tags from scratch.
                            _ => {
                                let n = 1 + (rng.next_u32() as usize % tag_ids.len().min(4));
                                tags = pick_tags(rng, tag_ids, n);
                            }
                        }
                    }
                }
                CardEdit::Blazed => {
                    blazed = !blazed;
                }
                CardEdit::DueDate => {
                    if due_date.is_some() && rng.next_u32().is_multiple_of(3) {
                        // Clear due date.
                        due_date = None;
                    } else {
                        // Regenerate due date biased toward now.
                        due_date = gen_due_date(rng, now).or(due_date);
                    }
                }
            }
        }

        chain.push(prev.next(content, priority, tags, blazed, modified_at, due_date));
    }

    chain
}

/// Generate `n` sorted random timestamps between `after` and `before`.
fn spread_timestamps(
    rng: &mut ChaCha8Rng,
    after: DateTime<Utc>,
    before: DateTime<Utc>,
    n: usize,
) -> Vec<DateTime<Utc>> {
    let total_secs = (before - after).num_seconds().max(n as i64);
    let mut times: Vec<DateTime<Utc>> = (0..n)
        .map(|_| {
            let offset = (rng.next_u64() % total_secs as u64) as i64;
            after + Duration::seconds(offset)
        })
        .collect();
    times.sort();
    times
}

/// Pick `n` unique edit types via partial Fisher-Yates.
fn pick_card_edits(rng: &mut ChaCha8Rng, n: usize) -> Vec<CardEdit> {
    let n = n.min(ALL_CARD_EDITS.len());
    let mut indices: Vec<usize> = (0..ALL_CARD_EDITS.len()).collect();
    for i in 0..n {
        let j = i + (rng.next_u32() as usize % (indices.len() - i));
        indices.swap(i, j);
    }
    indices[..n].iter().map(|&i| ALL_CARD_EDITS[i]).collect()
}

/// Pick `n` unique tags from the available tag IDs.
fn pick_tags(rng: &mut ChaCha8Rng, tag_ids: &[Uuid], n: usize) -> Vec<Uuid> {
    let n = n.min(tag_ids.len());
    // Simple Fisher-Yates on a copy of indices.
    let mut indices: Vec<usize> = (0..tag_ids.len()).collect();
    for i in 0..n {
        let j = i + (rng.next_u32() as usize % (indices.len() - i));
        indices.swap(i, j);
    }
    indices[..n].iter().map(|&i| tag_ids[i]).collect()
}

/// Capitalize the first alphabetic character of a string.
///
/// Non-alphabetic leading characters (e.g. `#` in Markdown headings) are
/// preserved unchanged.
fn capitalize_first(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalized = false;
    for c in s.chars() {
        if !capitalized && c.is_alphabetic() {
            result.extend(c.to_uppercase());
            capitalized = true;
        } else {
            result.push(c);
        }
    }
    result
}

/// Generate varied Markdown content for a card.
///
/// Cycles through several content patterns to exercise different Markdown features.
/// Biased toward longer content — most patterns produce multi-paragraph cards.
fn gen_card_content(rng: &mut ChaCha8Rng, index: usize) -> String {
    let content = match index % 8 {
        // Short-ish sentence
        0 => {
            let s: String = Sentence(8..20).fake_with_rng(rng);
            s
        }
        // Heading + multiple paragraphs separated by horizontal rules
        1 => {
            let heading: String = Sentence(4..8).fake_with_rng(rng);
            let p1: String = Paragraph(4..7).fake_with_rng(rng);
            let p2: String = Paragraph(3..6).fake_with_rng(rng);
            let p3: String = Paragraph(3..5).fake_with_rng(rng);
            format!("# {heading}\n\n{p1}\n\n---\n\n{p2}\n\n---\n\n{p3}")
        }
        // Long checklist (GFM task list)
        2 => {
            let heading: String = Sentence(4..8).fake_with_rng(rng);
            let n = 8 + (rng.next_u32() as usize % 12);
            let items: Vec<String> = (0..n)
                .map(|j| {
                    let text: String = Sentence(6..14).fake_with_rng(rng);
                    let checked = if j < n / 3 { "x" } else { " " };
                    format!("- [{checked}] {text}")
                })
                .collect();
            let note: String = Sentence(8..14).fake_with_rng(rng);
            format!("## {heading}\n\n{}\n\n{note}", items.join("\n"))
        }
        // Long bullet list with descriptions
        3 => {
            let heading: String = Sentence(3..7).fake_with_rng(rng);
            let n = 6 + (rng.next_u32() as usize % 10);
            let items: Vec<String> = (0..n)
                .map(|_| {
                    let title: String = Sentence(4..9).fake_with_rng(rng);
                    let desc: String = Sentence(8..16).fake_with_rng(rng);
                    format!("- **{title}** — {desc}")
                })
                .collect();
            format!("## {heading}\n\n{}", items.join("\n"))
        }
        // Code block with description and follow-up
        4 => {
            let intro: String = Paragraph(3..5).fake_with_rng(rng);
            let code_words: Vec<String> = Words(12..28).fake_with_rng(rng);
            let followup: String = Paragraph(3..5).fake_with_rng(rng);
            format!(
                "{intro}\n\n```\n{}\n```\n\n{followup}",
                code_words.join(" ")
            )
        }
        // Multi-paragraph essay
        5 => {
            let heading: String = Sentence(4..8).fake_with_rng(rng);
            let n = 4 + (rng.next_u32() as usize % 5);
            let paragraphs: Vec<String> = (0..n)
                .map(|_| {
                    let p: String = Paragraph(4..7).fake_with_rng(rng);
                    p
                })
                .collect();
            format!("# {heading}\n\n{}", paragraphs.join("\n\n"))
        }
        // Blockquote with context, separated by horizontal rules
        6 => {
            let context: String = Paragraph(3..5).fake_with_rng(rng);
            let quote: String = Sentence(10..22).fake_with_rng(rng);
            let analysis: String = Paragraph(3..5).fake_with_rng(rng);
            format!("{context}\n\n---\n\n> {quote}\n\n---\n\n{analysis}")
        }
        // Heading + bullet list + multiple notes
        7 => {
            let heading: String = Sentence(4..8).fake_with_rng(rng);
            let n = 5 + (rng.next_u32() as usize % 8);
            let items: Vec<String> = (0..n)
                .map(|_| {
                    let text: String = Sentence(6..12).fake_with_rng(rng);
                    format!("- {text}")
                })
                .collect();
            let note1: String = Sentence(8..16).fake_with_rng(rng);
            let note2: String = Sentence(6..12).fake_with_rng(rng);
            format!(
                "## {heading}\n\n{}\n\n**Note:** {note1}\n\n**See also:** {note2}",
                items.join("\n")
            )
        }
        _ => unreachable!(),
    };
    capitalize_first(&content)
}

/// Generate deleted cards with random priorities.
///
/// These cards will be pushed then immediately deleted in the same batch.
fn generate_deleted_cards(
    rng: &mut ChaCha8Rng,
    num_cards: usize,
    tag_ids: &[Uuid],
    base_time: DateTime<Utc>,
    used_priorities: &mut BTreeSet<i64>,
) -> Vec<Vec<Card>> {
    (0..num_cards)
        .map(|i| {
            let id = gen_uuid(rng);
            let content = gen_card_content(rng, i + 3);

            // Random priority across full i64 range, resolved to avoid collisions.
            let raw = rng.next_u64() as i64;
            let resolved = resolve_priority(raw, used_priorities);
            used_priorities.insert(resolved);
            let priority = resolved;

            let tags = if tag_ids.is_empty() || rng.next_u32().is_multiple_of(4) {
                vec![]
            } else {
                let n = 1 + (rng.next_u32() as usize % 3.min(tag_ids.len()));
                pick_tags(rng, tag_ids, n)
            };

            let blazed = rng.next_u32() % 100 < 20;
            let created_at = gen_timestamp(rng, base_time);
            let due_date = gen_due_date(rng, base_time);

            let first = Card::first(id, content, priority, tags, blazed, created_at, due_date);
            generate_card_history(rng, first, tag_ids, base_time, used_priorities)
        })
        .collect()
}

/// Create a single new card with random content, tags, and priority.
fn make_fresh_card(
    rng: &mut ChaCha8Rng,
    tag_ids: &[Uuid],
    now: DateTime<Utc>,
    used_priorities: &mut BTreeSet<i64>,
) -> Card {
    let id = gen_uuid(rng);
    let content_idx = rng.next_u32() as usize;
    let content = gen_card_content(rng, content_idx);
    let raw = rng.next_u64() as i64;
    let resolved = resolve_priority(raw, used_priorities);
    used_priorities.insert(resolved);
    let priority = resolved;
    let tags = if tag_ids.is_empty() || rng.next_u32().is_multiple_of(4) {
        vec![]
    } else {
        let n = 1 + (rng.next_u32() as usize % 3.min(tag_ids.len()));
        pick_tags(rng, tag_ids, n)
    };
    let blazed = rng.next_u32() % 100 < 30;
    let created_at = now - Duration::minutes((rng.next_u32() % 120) as i64);
    let due_date = gen_due_date(rng, now);
    Card::first(id, content, priority, tags, blazed, created_at, due_date)
}

/// Create a new version of an existing card with 1–2 random edits.
fn make_card_update(
    rng: &mut ChaCha8Rng,
    prev: &Card,
    tag_ids: &[Uuid],
    now: DateTime<Utc>,
    used_priorities: &mut BTreeSet<i64>,
) -> Card {
    let mut content = prev.content().to_string();
    let mut priority = prev.priority();
    let mut tags = prev.tags().to_vec();
    let mut blazed = prev.blazed();
    let mut due_date = prev.due_date();

    let num_changes = 1 + (rng.next_u32() as usize % 2);
    let edits = pick_card_edits(rng, num_changes);
    for edit in &edits {
        match edit {
            CardEdit::Content => {
                let note: String = Sentence(6..14).fake_with_rng(rng);
                content = format!("{content}\n\n**Update:** {note}");
            }
            CardEdit::Priority => {
                let shift = (rng.next_u64() % 1_000_000) as i64 - 500_000;
                let raw = priority.saturating_add(shift);
                used_priorities.remove(&priority);
                let resolved = resolve_priority(raw, used_priorities);
                used_priorities.insert(resolved);
                priority = resolved;
            }
            CardEdit::Tags => {
                if !tag_ids.is_empty() {
                    let new_tag = tag_ids[rng.next_u32() as usize % tag_ids.len()];
                    if !tags.contains(&new_tag) {
                        tags.push(new_tag);
                    }
                }
            }
            CardEdit::Blazed => blazed = !blazed,
            CardEdit::DueDate => due_date = gen_due_date(rng, now).or(due_date),
        }
    }

    prev.next(content, priority, tags, blazed, now, due_date)
}

/// Create a single new tag with a random title.
fn make_fresh_tag(rng: &mut ChaCha8Rng, now: DateTime<Utc>) -> Tag {
    let id = gen_uuid(rng);
    let words: Vec<String> = Words(1..3).fake_with_rng(rng);
    let title = capitalize_first(&words.join(" "));
    let color = random_tag_color(rng);
    let created_at = now - Duration::minutes((rng.next_u32() % 120) as i64);
    Tag::first(id, title, color, created_at)
}

/// Generate 120 extra operations for a rich, diverse sequence history.
///
/// Each inner `Vec<PushItem>` is pushed as a separate request, creating one
/// root sequence entry per element.  The operations flow through five phases:
///
/// A. Create entities destined for deletion (purged-history scenario)
/// B. Mix of individual creates and updates
/// C. Small batches of 2–5 mixed operations
/// D. Delete the doomed entities (shows "operations purged")
/// E. Post-deletion activity (proves life goes on)
fn generate_extra_ops(
    rng: &mut ChaCha8Rng,
    live_tag_ids: &[Uuid],
    card_chains: &[Vec<Card>],
    tag_chains: &[Vec<Tag>],
    base_time: DateTime<Utc>,
    used_priorities: &mut BTreeSet<i64>,
) -> Vec<Vec<PushItem>> {
    // Need both cards and tags as a base for updates.
    if card_chains.is_empty() || tag_chains.is_empty() {
        return Vec::new();
    }

    let mut ops: Vec<Vec<PushItem>> = Vec::with_capacity(120);

    // Track latest versions so multi-update doesn't collide on count.
    let mut card_heads: Vec<Card> = card_chains
        .iter()
        .map(|chain| chain.last().unwrap().clone())
        .collect();
    let mut tag_heads: Vec<Tag> = tag_chains
        .iter()
        .map(|chain| chain.last().unwrap().clone())
        .collect();

    let mut card_update_idx = 0usize;
    let mut tag_update_idx = 0usize;

    // Entities created for later deletion.
    let mut doomed_card_ids: Vec<Uuid> = Vec::new();
    let mut doomed_tag_ids: Vec<Uuid> = Vec::new();

    // ── Phase A: Create entities destined for deletion (20 ops) ──

    for _ in 0..15 {
        let card = make_fresh_card(rng, live_tag_ids, base_time, used_priorities);
        doomed_card_ids.push(card.id());
        ops.push(vec![PushItem::Cards(vec![card])]);
    }
    for _ in 0..5 {
        let tag = make_fresh_tag(rng, base_time);
        doomed_tag_ids.push(tag.id());
        ops.push(vec![PushItem::Tags(vec![tag])]);
    }

    // ── Phase B: Individual creates and updates (58 ops) ──
    // No new live tags — only card creates/updates + tag updates.

    // 25 new card creates
    for _ in 0..25 {
        let card = make_fresh_card(rng, live_tag_ids, base_time, used_priorities);
        ops.push(vec![PushItem::Cards(vec![card])]);
    }

    // 15 card updates
    for _ in 0..15 {
        let idx = card_update_idx % card_heads.len();
        card_update_idx += 1;
        let updated = make_card_update(
            rng,
            &card_heads[idx],
            live_tag_ids,
            base_time,
            used_priorities,
        );
        card_heads[idx] = updated.clone();
        ops.push(vec![PushItem::Cards(vec![updated])]);
    }

    // 10 more card creates
    for _ in 0..10 {
        let card = make_fresh_card(rng, live_tag_ids, base_time, used_priorities);
        ops.push(vec![PushItem::Cards(vec![card])]);
    }

    // 8 tag updates
    for _ in 0..8 {
        let idx = tag_update_idx % tag_heads.len();
        tag_update_idx += 1;
        let prev = &tag_heads[idx];
        let updated = prev.next(mutate_tag_title(rng, prev.title()), prev.color(), base_time);
        tag_heads[idx] = updated.clone();
        ops.push(vec![PushItem::Tags(vec![updated])]);
    }

    // ── Phase C: Small batches of 2–5 mixed operations (12 ops) ──

    for _ in 0..12 {
        let batch_size = 2 + (rng.next_u32() as usize % 4);
        let mut batch = Vec::with_capacity(batch_size);
        for _ in 0..batch_size {
            match rng.next_u32() % 4 {
                0 => {
                    batch.push(PushItem::Cards(vec![make_fresh_card(
                        rng,
                        live_tag_ids,
                        base_time,
                        used_priorities,
                    )]));
                }
                1 => {
                    let idx = card_update_idx % card_heads.len();
                    card_update_idx += 1;
                    let updated = make_card_update(
                        rng,
                        &card_heads[idx],
                        live_tag_ids,
                        base_time,
                        used_priorities,
                    );
                    card_heads[idx] = updated.clone();
                    batch.push(PushItem::Cards(vec![updated]));
                }
                2 => {
                    batch.push(PushItem::Cards(vec![make_fresh_card(
                        rng,
                        live_tag_ids,
                        base_time,
                        used_priorities,
                    )]));
                }
                _ => {
                    let idx = tag_update_idx % tag_heads.len();
                    tag_update_idx += 1;
                    let prev = &tag_heads[idx];
                    let updated =
                        prev.next(mutate_tag_title(rng, prev.title()), prev.color(), base_time);
                    tag_heads[idx] = updated.clone();
                    batch.push(PushItem::Tags(vec![updated]));
                }
            }
        }
        ops.push(batch);
    }

    // ── Phase D: Delete doomed entities (20 ops) ──

    for id in &doomed_card_ids {
        ops.push(vec![PushItem::DeleteCard { id: *id }]);
    }
    for id in &doomed_tag_ids {
        // Remove the doomed tag from any live cards that reference it
        let mut batch: Vec<PushItem> = Vec::new();
        for head in card_heads.iter_mut() {
            if head.tags().contains(id) {
                let new_tags: Vec<Uuid> =
                    head.tags().iter().copied().filter(|t| t != id).collect();
                let updated = head.next(
                    head.content().to_string(),
                    head.priority(),
                    new_tags,
                    head.blazed(),
                    base_time,
                    head.due_date(),
                );
                *head = updated.clone();
                batch.push(PushItem::Cards(vec![updated]));
            }
        }
        batch.push(PushItem::DeleteTag { id: *id });
        ops.push(batch);
    }

    // ── Phase E: Post-deletion activity (10 ops) ──

    for _ in 0..10 {
        match rng.next_u32() % 4 {
            0 => {
                ops.push(vec![PushItem::Cards(vec![make_fresh_card(
                    rng,
                    live_tag_ids,
                    base_time,
                    used_priorities,
                )])]);
            }
            1 => {
                let idx = card_update_idx % card_heads.len();
                card_update_idx += 1;
                let updated = make_card_update(
                    rng,
                    &card_heads[idx],
                    live_tag_ids,
                    base_time,
                    used_priorities,
                );
                card_heads[idx] = updated.clone();
                ops.push(vec![PushItem::Cards(vec![updated])]);
            }
            2 => {
                let idx = tag_update_idx % tag_heads.len();
                tag_update_idx += 1;
                let prev = &tag_heads[idx];
                let updated =
                    prev.next(mutate_tag_title(rng, prev.title()), prev.color(), base_time);
                tag_heads[idx] = updated.clone();
                ops.push(vec![PushItem::Tags(vec![updated])]);
            }
            _ => {
                // Mixed batch: two new cards in one sequence.
                let card1 = make_fresh_card(rng, live_tag_ids, base_time, used_priorities);
                let card2 = make_fresh_card(rng, live_tag_ids, base_time, used_priorities);
                ops.push(vec![
                    PushItem::Cards(vec![card1]),
                    PushItem::Cards(vec![card2]),
                ]);
            }
        }
    }

    ops
}

/// Inject card links into some cards by appending a new version with UUIDs
/// embedded in the content.
///
/// Distribution (of the full card set):
/// - ~50%: no links
/// - ~20%: one link to another card
/// - ~15%: 2–3 links to other cards
/// - ~10%: same UUID referenced twice (demonstrates deduplication)
/// - ~5%: self-reference + another card (demonstrates self-exclusion)
fn inject_card_links(rng: &mut ChaCha8Rng, card_chains: &mut [Vec<Card>]) {
    let card_ids: Vec<Uuid> = card_chains.iter().map(|c| c[0].id()).collect();
    if card_ids.len() < 2 {
        return;
    }

    for i in 0..card_chains.len() {
        let roll = rng.next_u32() % 100;
        if roll < 50 {
            continue;
        }

        let own_id = card_ids[i];
        let prev = card_chains[i].last().unwrap();

        let link_text = if roll < 70 {
            // One link.
            let target = pick_other_id(rng, &card_ids, own_id);
            format!("\n\nSee also: {target}")
        } else if roll < 85 {
            // 2–3 links.
            let n = 2 + (rng.next_u32() as usize % 2);
            let targets: Vec<String> = (0..n)
                .map(|_| pick_other_id(rng, &card_ids, own_id).to_string())
                .collect();
            format!("\n\nRelated: {}", targets.join(", "))
        } else if roll < 95 {
            // Same UUID twice — shows deduplication.
            let target = pick_other_id(rng, &card_ids, own_id);
            format!("\n\nRelated: {target}\nAlso see: {target}")
        } else {
            // Self-reference + another — shows self-exclusion.
            let target = pick_other_id(rng, &card_ids, own_id);
            format!("\n\nThis card: {own_id}\nRelated: {target}")
        };

        let new_content = format!("{}{link_text}", prev.content());
        let modified_at = prev.modified_at() + Duration::seconds(1);

        let new_version = prev.next(
            new_content,
            prev.priority(),
            prev.tags().to_vec(),
            prev.blazed(),
            modified_at,
            prev.due_date(),
        );
        card_chains[i].push(new_version);
    }
}

/// Pick a random card ID that is not `exclude`.
fn pick_other_id(rng: &mut ChaCha8Rng, ids: &[Uuid], exclude: Uuid) -> Uuid {
    loop {
        let idx = rng.next_u32() as usize % ids.len();
        if ids[idx] != exclude {
            return ids[idx];
        }
    }
}

/// Generate deterministic seed data: live entities plus deleted entities.
pub fn generate(seed: u64, num_tags: usize, num_cards: usize) -> SeedData {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Anchor to now so seeded data has realistic, recent-looking timestamps.
    let base_time = Utc::now();

    // Track every priority ever assigned so collisions can be resolved
    // deterministically (midpoint of the nearest gap, inspired by the
    // client lib's placement logic but without non-deterministic jitter).
    let mut used_priorities = BTreeSet::new();

    let tag_chains = generate_tags(&mut rng, num_tags, base_time);
    let tag_ids: Vec<Uuid> = tag_chains
        .iter()
        .map(|chain| chain.first().unwrap().id())
        .collect();

    let mut card_chains = generate_cards(
        &mut rng,
        num_cards,
        &tag_ids,
        base_time,
        &mut used_priorities,
    );

    // Inject card links into ~50% of cards so the UI has linked-card data.
    inject_card_links(&mut rng, &mut card_chains);

    // Extra entities that get created then deleted (~75% cards, 3-5 tags).
    let num_deleted_cards = num_cards * 3 / 4;
    let num_deleted_tags = (num_tags / 4).clamp(3, 5);

    let deleted_tag_chains = generate_tags(&mut rng, num_deleted_tags, base_time);
    let deleted_card_chains = generate_deleted_cards(
        &mut rng,
        num_deleted_cards,
        &tag_ids,
        base_time,
        &mut used_priorities,
    );

    // 120 extra operations pushed individually for a diverse sequence history.
    let extra_ops = generate_extra_ops(
        &mut rng,
        &tag_ids,
        &card_chains,
        &tag_chains,
        base_time,
        &mut used_priorities,
    );

    SeedData {
        tag_chains,
        card_chains,
        deleted_tag_chains,
        deleted_card_chains,
        extra_ops,
    }
}
