use super::RootState;
use expect_test::expect;

#[test]
fn empty_root_has_zero_hash_and_sequence() {
    let root = RootState::empty();
    expect!["0000000000000000000000000000000000000000000000000000000000000000"]
        .assert_eq(&root.hash.to_string());
    expect!["0"].assert_eq(&root.sequence.to_string());
}
