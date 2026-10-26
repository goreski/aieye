//! The definitions shipped in the repository must all load.

use std::path::Path;

#[test]
fn shipped_definitions_load() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../definitions");
    let definitions = aieye_defs::load_dir(&dir).unwrap();
    assert!(!definitions.is_empty());
}
