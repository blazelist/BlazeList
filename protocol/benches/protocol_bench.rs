use criterion::{Criterion, criterion_group, criterion_main};

use blazelist_protocol::{Card, DateTime, DeletedEntity, NonNegativeI64, RootState, Utc};
use blazelist_protocol::{CardFilter, Request, Response};
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

fn p(v: i64) -> NonNegativeI64 {
    NonNegativeI64::try_from(v).unwrap()
}

fn sample_card() -> Card {
    Card::first(
        ID,
        "Buy groceries\n- Tofu\n- Lentils\n- Bread".into(),
        p(5_000_000_000),
        vec![TAG_ID],
        false,
        ts(1_000_000),
        None,
    )
}

// -- Request serialization ----------------------------------------------------

fn bench_request_push_card_serialize(c: &mut Criterion) {
    let card = sample_card();
    let req = Request::PushCardVersions(vec![card]);
    c.bench_function("request_push_card_serialize", |b| {
        b.iter(|| postcard::to_allocvec(&req).unwrap());
    });
}

fn bench_request_push_card_round_trip(c: &mut Criterion) {
    let card = sample_card();
    let req = Request::PushCardVersions(vec![card]);
    let bytes = postcard::to_allocvec(&req).unwrap();
    c.bench_function("request_push_card_round_trip", |b| {
        b.iter(|| {
            let serialized = postcard::to_allocvec(&req).unwrap();
            let _: Request = postcard::from_bytes(&serialized).unwrap();
            let _ = &bytes; // keep bytes alive
        });
    });
}

fn bench_request_list_cards_round_trip(c: &mut Criterion) {
    let req = Request::ListCards {
        filter: CardFilter::Extinguished,
        limit: None,
    };
    c.bench_function("request_list_cards_round_trip", |b| {
        b.iter(|| {
            let bytes = postcard::to_allocvec(&req).unwrap();
            let _: Request = postcard::from_bytes(&bytes).unwrap();
        });
    });
}

// -- Response serialization ---------------------------------------------------

fn bench_response_cards_serialize(c: &mut Criterion) {
    let cards: Vec<Card> = (0..100u8)
        .map(|i| {
            Card::first(
                Uuid::from_bytes([i; 16]),
                format!("Card {i}\nContent for card {i}"),
                p(i as i64 * 1000),
                vec![],
                false,
                ts(i as i64 * 1000),
                None,
            )
        })
        .collect();
    let resp = Response::Cards(cards);
    c.bench_function("response_100_cards_serialize", |b| {
        b.iter(|| postcard::to_allocvec(&resp).unwrap());
    });
}

fn bench_response_cards_deserialize(c: &mut Criterion) {
    let cards: Vec<Card> = (0..100u8)
        .map(|i| {
            Card::first(
                Uuid::from_bytes([i; 16]),
                format!("Card {i}\nContent for card {i}"),
                p(i as i64 * 1000),
                vec![],
                false,
                ts(i as i64 * 1000),
                None,
            )
        })
        .collect();
    let resp = Response::Cards(cards);
    let bytes = postcard::to_allocvec(&resp).unwrap();
    c.bench_function("response_100_cards_deserialize", |b| {
        b.iter(|| {
            let _: Response = postcard::from_bytes(&bytes).unwrap();
        });
    });
}

fn bench_response_root_round_trip(c: &mut Criterion) {
    let resp = Response::Root(RootState::empty());
    c.bench_function("response_root_round_trip", |b| {
        b.iter(|| {
            let bytes = postcard::to_allocvec(&resp).unwrap();
            let _: Response = postcard::from_bytes(&bytes).unwrap();
        });
    });
}

fn bench_response_deleted_round_trip(c: &mut Criterion) {
    let resp = Response::Deleted(DeletedEntity::new(ID));
    c.bench_function("response_deleted_round_trip", |b| {
        b.iter(|| {
            let bytes = postcard::to_allocvec(&resp).unwrap();
            let _: Response = postcard::from_bytes(&bytes).unwrap();
        });
    });
}

// -- Scaling: many cards in a single response ---------------------------------

fn bench_response_cards_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("response_cards_scaling");

    for &count in SCALING_TIERS {
        let cards: Vec<Card> = (0..count)
            .map(|i| {
                let id_bytes: [u8; 16] = {
                    let mut b = [0u8; 16];
                    let bytes = i.to_le_bytes();
                    b[..4].copy_from_slice(&bytes);
                    b
                };
                Card::first(
                    Uuid::from_bytes(id_bytes),
                    format!("Card {i}\nContent for card {i}"),
                    p(i as i64 * 1000),
                    vec![],
                    false,
                    ts(i as i64 * 1000),
                    None,
                )
            })
            .collect();
        let resp = Response::Cards(cards);
        let bytes = postcard::to_allocvec(&resp).unwrap();

        // For large tiers, reduce sample size
        if count >= 100_000 {
            group.sample_size(10);
        }

        group.bench_function(format!("serialize_{count}_cards"), |b| {
            b.iter(|| postcard::to_allocvec(&resp).unwrap());
        });
        group.bench_function(format!("deserialize_{count}_cards"), |b| {
            b.iter(|| {
                let _: Response = postcard::from_bytes(&bytes).unwrap();
            });
        });
    }
    group.finish();
}

criterion_group!(
    requests,
    bench_request_push_card_serialize,
    bench_request_push_card_round_trip,
    bench_request_list_cards_round_trip,
);

criterion_group!(
    responses,
    bench_response_cards_serialize,
    bench_response_cards_deserialize,
    bench_response_root_round_trip,
    bench_response_deleted_round_trip,
);

criterion_group!(scaling, bench_response_cards_scaling);

criterion_main!(requests, responses, scaling);
