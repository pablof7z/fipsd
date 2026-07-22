#![allow(dead_code)]

use std::path::PathBuf;

pub fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(repository().join("fixtures/m5/chaos").join(name)).unwrap()
}
