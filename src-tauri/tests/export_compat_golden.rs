//! Frozen compatibility coverage for the stable static-publishing package.

#![cfg(feature = "test-support")]

#[test]
fn frozen_v1_package_is_still_readable() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/static-publish/v1");
    cull_lib::test_support::validate_cull_static_package_for_test(&fixture)
        .expect("current Cull must read the frozen stable v1 package");
}
