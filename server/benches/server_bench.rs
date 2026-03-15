use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};

use blazelist_protocol::{Card, DateTime, Entity, NonNegativeI64, Tag, Utc};
use blazelist_protocol::{CardFilter, PushItem};
use blazelist_server::{SqliteStorage, Storage};
use uuid::Uuid;

// Shared scaling tiers for all benchmarks: 10, 100, 1K, 10K, 100K
const SCALING_TIERS: &[u32] = &[10, 100, 1_000, 10_000, 100_000];

fn ts(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap()
}

fn make_card(i: u32) -> Card {
    let mut id_bytes = [0u8; 16];
    id_bytes[..4].copy_from_slice(&i.to_le_bytes());
    Card::first(
        Uuid::from_bytes(id_bytes),
        format!("Content for card {i}"),
        i as i64 * 1000,
        vec![],
        false,
        ts(i as i64 * 1000),
        None,
    )
}

fn make_tag(i: u32) -> Tag {
    let mut id_bytes = [0u8; 16];
    id_bytes[..4].copy_from_slice(&i.to_le_bytes());
    Tag::first(
        Uuid::from_bytes(id_bytes),
        format!("Tag {i}"),
        None,
        ts(i as i64 * 1000),
    )
}

fn populated_store(num_cards: u32) -> SqliteStorage {
    let store = SqliteStorage::open_in_memory().unwrap();

    // For small counts, use individual pushes (simpler, faster for small N)
    // For large counts, use batch push to avoid O(N²) root recomputation cost
    if num_cards <= 1_000 {
        for i in 0..num_cards {
            store.push_card_versions(&[make_card(i)]).unwrap();
        }
    } else {
        // Batch push all cards at once - only one root recomputation
        let cards: Vec<PushItem> = (0..num_cards)
            .map(|i| PushItem::Cards(vec![make_card(i)]))
            .collect();
        store.push_batch(&cards).unwrap();
    }

    store
}

// -- Push benchmarks ----------------------------------------------------------

fn bench_push_card_single(c: &mut Criterion) {
    c.bench_function("push_card_single", |b| {
        b.iter_with_setup(
            || {
                let store = SqliteStorage::open_in_memory().unwrap();
                let card = make_card(0);
                (store, card)
            },
            |(store, card)| {
                store.push_card_versions(&[card]).unwrap();
            },
        );
    });
}

fn bench_push_card_version(c: &mut Criterion) {
    c.bench_function("push_card_version", |b| {
        b.iter_with_setup(
            || {
                let store = SqliteStorage::open_in_memory().unwrap();
                let card = make_card(0);
                store
                    .push_card_versions(std::slice::from_ref(&card))
                    .unwrap();
                let next = card.next(
                    "Updated content".into(),
                    2000,
                    vec![],
                    false,
                    ts(2000),
                    None,
                );
                (store, next)
            },
            |(store, next)| {
                store.push_card_versions(&[next]).unwrap();
            },
        );
    });
}

// -- Get benchmarks -----------------------------------------------------------

fn bench_get_card(c: &mut Criterion) {
    let store = populated_store(100);
    let mut id_bytes = [0u8; 16];
    id_bytes[..4].copy_from_slice(&50u32.to_le_bytes());
    let target_id = Uuid::from_bytes(id_bytes);

    c.bench_function("get_card", |b| {
        b.iter(|| {
            store.get_card(target_id).unwrap().unwrap();
        });
    });
}

fn bench_get_root(c: &mut Criterion) {
    let store = populated_store(100);
    c.bench_function("get_root", |b| {
        b.iter(|| {
            store.get_root().unwrap();
        });
    });
}

// -- List benchmarks (scaling) ------------------------------------------------

fn bench_list_cards_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_cards");

    for &count in SCALING_TIERS {
        let store = populated_store(count);

        // For large tiers, reduce sample size to keep benchmark time reasonable
        if count >= 100_000 {
            group.sample_size(10);
        }

        group.bench_function(format!("list_all_{count}"), |b| {
            b.iter(|| {
                store.list_cards(CardFilter::All, None).unwrap();
            });
        });
    }
    group.finish();
}

fn bench_list_cards_filtered(c: &mut Criterion) {
    let store = SqliteStorage::open_in_memory().unwrap();
    for i in 0..500u32 {
        let mut id_bytes = [0u8; 16];
        id_bytes[..4].copy_from_slice(&i.to_le_bytes());
        let card = Card::first(
            Uuid::from_bytes(id_bytes),
            format!("Content {i}"),
            i as i64 * 1000,
            vec![],
            i % 2 == 0, // half blazed, half extinguished
            ts(i as i64 * 1000),
            None,
        );
        store.push_card_versions(&[card]).unwrap();
    }

    let mut group = c.benchmark_group("list_cards_filtered");
    group.bench_function("blazed_250_of_500", |b| {
        b.iter(|| {
            store.list_cards(CardFilter::Blazed, None).unwrap();
        });
    });
    group.bench_function("extinguished_250_of_500", |b| {
        b.iter(|| {
            store.list_cards(CardFilter::Extinguished, None).unwrap();
        });
    });
    group.finish();
}

// -- Root recomputation scaling -----------------------------------------------
// Root recomputation uses a 256-bucket scheme: on mutation, only the affected
// bucket (1/256th of entities) is recomputed, then the root is derived from
// 256 cached bucket hashes. Cost per mutation is O(N/256 + 256) instead of
// O(N), so push latency should be roughly constant across scaling tiers.

fn bench_root_recomputation_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("root_recomputation");

    for &existing in SCALING_TIERS {
        // For large tiers, reduce sample size and extend measurement time
        if existing >= 100_000 {
            group.sample_size(10);
            group.measurement_time(Duration::from_secs(30));
        }

        group.bench_function(format!("push_into_{existing}_cards"), |b| {
            b.iter_with_setup(
                || {
                    let store = populated_store(existing);
                    let card = make_card(existing);
                    (store, card)
                },
                |(store, card)| {
                    store.push_card_versions(&[card]).unwrap();
                },
            );
        });
    }
    group.finish();
}

// -- Card history scaling -----------------------------------------------------

fn bench_card_history(c: &mut Criterion) {
    let mut group = c.benchmark_group("card_history");

    for &versions in SCALING_TIERS {
        let store = SqliteStorage::open_in_memory().unwrap();
        let mut card = make_card(0);
        store.push_card_versions(&[card.clone()]).unwrap();
        for i in 1..versions {
            let next = card.next(
                format!("Content v{i}"),
                i as i64 * 1000,
                vec![],
                false,
                ts(i as i64 * 1000),
                None,
            );
            store
                .push_card_versions(std::slice::from_ref(&next))
                .unwrap();
            card = next;
        }

        // For large tiers, reduce sample size
        if versions >= 100_000 {
            group.sample_size(10);
        }

        group.bench_function(format!("get_history_{versions}_versions"), |b| {
            b.iter(|| {
                store.get_card_history(card.id(), None).unwrap();
            });
        });
    }
    group.finish();
}

// -- Tag benchmarks -----------------------------------------------------------

fn bench_push_tag(c: &mut Criterion) {
    c.bench_function("push_tag_single", |b| {
        b.iter_with_setup(
            || {
                let store = SqliteStorage::open_in_memory().unwrap();
                let tag = make_tag(0);
                (store, tag)
            },
            |(store, tag)| {
                store.push_tag_versions(&[tag]).unwrap();
            },
        );
    });
}

fn bench_list_tags(c: &mut Criterion) {
    let store = SqliteStorage::open_in_memory().unwrap();
    for i in 0..100u32 {
        store.push_tag_versions(&[make_tag(i)]).unwrap();
    }
    c.bench_function("list_100_tags", |b| {
        b.iter(|| {
            store.list_tags().unwrap();
        });
    });
}

// -- Sync benchmarks ----------------------------------------------------------

fn bench_get_changes_since_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_changes_since");

    for &count in SCALING_TIERS {
        let store = populated_store(count);

        // For large tiers, reduce sample size
        if count >= 100_000 {
            group.sample_size(10);
        }

        // Test full sync (get all changes from sequence 0)
        group.bench_function(format!("full_sync_{count}_cards"), |b| {
            b.iter(|| {
                store
                    .get_changes_since(NonNegativeI64::MIN, blake3::Hash::from([0u8; 32]))
                    .unwrap();
            });
        });

        // Test incremental sync (get last ~10% of changes)
        // Calculate sequence number for 90% of cards
        let ninety_percent_seq = if count > 10 {
            NonNegativeI64::try_from((count as i64 * 9) / 10).unwrap()
        } else {
            NonNegativeI64::MIN
        };

        // For incremental sync, we need to get the hash at that sequence
        // Since we're just benchmarking and not validating state, we'll use a placeholder hash
        // In a real scenario, the client would have the actual hash from their last sync
        group.bench_function(format!("incremental_10pct_{count}_cards"), |b| {
            b.iter(|| {
                // This will fail hash validation but still exercises the query path
                // The benchmark measures the query performance, not validation
                let _ = store.get_changes_since(ninety_percent_seq, blake3::Hash::from([0u8; 32]));
            });
        });
    }
    group.finish();
}

criterion_group!(
    push,
    bench_push_card_single,
    bench_push_card_version,
    bench_push_tag,
);

criterion_group!(read, bench_get_card, bench_get_root, bench_list_tags,);

criterion_group!(
    scaling,
    bench_list_cards_scaling,
    bench_list_cards_filtered,
    bench_root_recomputation_scaling,
    bench_card_history,
    bench_get_changes_since_scaling,
);

criterion_main!(push, read, scaling);
