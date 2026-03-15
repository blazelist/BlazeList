use criterion::{Criterion, criterion_group, criterion_main};

use blazelist_protocol::{Card, DeletedEntity, Entity, NonNegativeI64, Tag};
use chrono::{DateTime, Utc};
use uuid::Uuid;

// Shared scaling tiers for all benchmarks: 10, 100, 1K, 10K, 100K
const SCALING_TIERS: &[u32] = &[10, 100, 1_000, 10_000, 100_000];

const ID: Uuid = Uuid::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
const TAG_ID: Uuid = Uuid::from_bytes([
    0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0x47, 0x08, 0x89, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
]);

fn ts(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap()
}

fn sample_card() -> Card {
    Card::first(
        ID,
        "Buy groceries\n- Tofu\n- Lentils\n- Bread\n- Tahini\n- Hummus".into(),
        5_000_000_000,
        vec![TAG_ID],
        false,
        ts(1_000_000),
        None,
    )
}

// -- BLAKE3 hashing benchmarks ------------------------------------------------

fn bench_card_hash_computation(c: &mut Criterion) {
    let card = sample_card();
    c.bench_function("card_expected_hash", |b| {
        b.iter(|| card.expected_hash());
    });
}

fn bench_card_first(c: &mut Criterion) {
    c.bench_function("card_first", |b| {
        b.iter(|| {
            Card::first(
                ID,
                "Buy groceries\n- Tofu\n- Lentils\n- Bread".into(),
                5_000_000_000,
                vec![TAG_ID],
                false,
                ts(1_000_000),
                None,
            )
        });
    });
}

fn bench_card_next(c: &mut Criterion) {
    let card = sample_card();
    c.bench_function("card_next", |b| {
        b.iter(|| {
            card.next(
                "Updated content with more text".into(),
                5_000_000_001,
                vec![TAG_ID],
                false,
                ts(2_000_000),
                None,
            )
        });
    });
}

fn bench_card_verify(c: &mut Criterion) {
    let card = sample_card();
    c.bench_function("card_verify", |b| {
        b.iter(|| card.verify());
    });
}

fn bench_tag_first(c: &mut Criterion) {
    c.bench_function("tag_first", |b| {
        b.iter(|| Tag::first(TAG_ID, "Groceries".into(), None, ts(1_000_000)));
    });
}

fn bench_tag_next(c: &mut Criterion) {
    let tag = Tag::first(TAG_ID, "Groceries".into(), None, ts(1_000_000));
    c.bench_function("tag_next", |b| {
        b.iter(|| tag.next("Food".into(), None, ts(2_000_000)));
    });
}

fn bench_deleted_entity_new(c: &mut Criterion) {
    c.bench_function("deleted_entity_new", |b| {
        b.iter(|| DeletedEntity::new(ID));
    });
}

fn bench_deleted_entity_verify(c: &mut Criterion) {
    let entity = DeletedEntity::new(ID);
    c.bench_function("deleted_entity_verify", |b| {
        b.iter(|| entity.verify());
    });
}

// -- Hash chain benchmarks ----------------------------------------------------

fn bench_hash_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_chain");

    for &chain_len in SCALING_TIERS {
        // For large tiers, reduce sample size
        if chain_len >= 100_000 {
            group.sample_size(10);
        }

        group.bench_function(format!("chain_{chain_len}_versions"), |b| {
            b.iter(|| {
                let mut card =
                    Card::first(ID, "Content".into(), 1000, vec![], false, ts(0), None);
                for i in 1..chain_len {
                    card = card.next(
                        format!("Content version {i}"),
                        1000,
                        vec![],
                        false,
                        ts(i as i64 * 1000),
                        None,
                    );
                }
            });
        });
    }
    group.finish();
}

// -- Serialization benchmarks -------------------------------------------------

fn bench_card_postcard_serialize(c: &mut Criterion) {
    let card = sample_card();
    c.bench_function("card_postcard_serialize", |b| {
        b.iter(|| postcard::to_allocvec(&card).unwrap());
    });
}

fn bench_card_postcard_deserialize(c: &mut Criterion) {
    let card = sample_card();
    let bytes = postcard::to_allocvec(&card).unwrap();
    c.bench_function("card_postcard_deserialize", |b| {
        b.iter(|| {
            let _: Card = postcard::from_bytes(&bytes).unwrap();
        });
    });
}

fn bench_card_postcard_round_trip(c: &mut Criterion) {
    let card = sample_card();
    c.bench_function("card_postcard_round_trip", |b| {
        b.iter(|| {
            let bytes = postcard::to_allocvec(&card).unwrap();
            let _: Card = postcard::from_bytes(&bytes).unwrap();
        });
    });
}

fn bench_tag_postcard_round_trip(c: &mut Criterion) {
    let tag = Tag::first(TAG_ID, "Groceries".into(), None, ts(1_000_000));
    c.bench_function("tag_postcard_round_trip", |b| {
        b.iter(|| {
            let bytes = postcard::to_allocvec(&tag).unwrap();
            let _: Tag = postcard::from_bytes(&bytes).unwrap();
        });
    });
}

// -- NonNegativeI64 benchmarks ------------------------------------------------

fn bench_non_negative_i64_try_from_i64(c: &mut Criterion) {
    c.bench_function("non_negative_i64_try_from_i64", |b| {
        b.iter(|| NonNegativeI64::try_from(42i64).unwrap());
    });
}

fn bench_non_negative_i64_try_from_u64(c: &mut Criterion) {
    c.bench_function("non_negative_i64_try_from_u64", |b| {
        b.iter(|| NonNegativeI64::try_from(42u64).unwrap());
    });
}

criterion_group!(
    hashing,
    bench_card_hash_computation,
    bench_card_first,
    bench_card_next,
    bench_card_verify,
    bench_tag_first,
    bench_tag_next,
    bench_deleted_entity_new,
    bench_deleted_entity_verify,
);

criterion_group!(chain, bench_hash_chain);

criterion_group!(
    serialization,
    bench_card_postcard_serialize,
    bench_card_postcard_deserialize,
    bench_card_postcard_round_trip,
    bench_tag_postcard_round_trip,
);

criterion_group!(
    priority,
    bench_non_negative_i64_try_from_i64,
    bench_non_negative_i64_try_from_u64,
);

criterion_main!(hashing, chain, serialization, priority);
