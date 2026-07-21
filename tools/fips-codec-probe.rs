use fips::proto::fmp::wire::{
    build_encrypted, build_established_header, prepend_inner_header, ESTABLISHED_HEADER_SIZE,
};
use fips::{
    BloomFilter, CoordsRequired, CoordEntry, FilterAnnounce, LookupRequest, LookupResponse,
    MtuExceeded, NodeAddr, ParentDeclaration, SessionAck, SessionSetup, TreeAnnounce,
    TreeCoordinate,
};
use secp256k1::schnorr::Signature;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use fips::utils::index::SessionIndex;

const FIPS_COMMIT: &str = "80c956a6fdb85dde1450969a21891c1158e43267";

#[derive(Serialize)]
struct TreeBoundary {
    depth: u32,
    message_bytes: u64,
    framed_bytes: u64,
}

#[derive(Serialize)]
struct Manifest {
    fips_commit: &'static str,
    source_sha256: BTreeMap<&'static str, String>,
    fmp_established_header_bytes: u64,
    fmp_timestamp_bytes: u64,
    aead_tag_bytes: u64,
    tree_announce: Vec<TreeBoundary>,
    encoded_sizes: BTreeMap<&'static str, u64>,
    maximum_safe_tree_depth: u32,
}

fn main() {
    let depths = [0, 35, 64, 65, 2000, maximum_safe_tree_depth()];
    let tree_announce = depths
        .into_iter()
        .map(|depth| {
            let message = tree_announce(depth);
            let frame = fmp_frame(&message);
            TreeBoundary {
                depth,
                message_bytes: message.len() as u64,
                framed_bytes: frame.len() as u64,
            }
        })
        .collect();

    let filter = BloomFilter::new();
    let filter_message = FilterAnnounce::new(filter, 7).encode().unwrap();
    let root = TreeCoordinate::root(node(1));
    let lookup_request = LookupRequest::new(1, node(2), node(1), root.clone(), 64, 1280);
    let signature = Signature::from_slice(&[1_u8; 64]).unwrap();
    let lookup_response = LookupResponse::new(1, node(2), root.clone(), signature);
    let coords_required = CoordsRequired::new(node(2), node(3));
    let mtu_exceeded = MtuExceeded::new(node(2), node(3), 1280);
    let session_setup = SessionSetup::new(root.clone(), root.clone()).with_handshake(vec![0; 33]);
    let session_ack = SessionAck::new(root.clone(), root).with_handshake(vec![0; 57]);

    let mut encoded_sizes = BTreeMap::new();
    encoded_sizes.insert("filter_announce_message", filter_message.len() as u64);
    encoded_sizes.insert("filter_announce_fmp_frame", fmp_frame(&filter_message).len() as u64);
    encoded_sizes.insert("lookup_request_depth_0", lookup_request.encode().len() as u64);
    encoded_sizes.insert("lookup_response_depth_0", lookup_response.encode().len() as u64);
    encoded_sizes.insert("routing_coords_required", coords_required.encode().len() as u64);
    encoded_sizes.insert("routing_mtu_exceeded", mtu_exceeded.encode().len() as u64);
    encoded_sizes.insert(
        "fsp_session_setup_depth_0_msg1_33",
        session_setup.encode().len() as u64,
    );
    encoded_sizes.insert(
        "fsp_session_ack_depth_0_msg2_57",
        session_ack.encode().len() as u64,
    );
    encoded_sizes.insert("fmp_established_zero_message_bytes", fmp_frame(&[]).len() as u64);

    let manifest = Manifest {
        fips_commit: FIPS_COMMIT,
        source_sha256: source_hashes(),
        fmp_established_header_bytes: ESTABLISHED_HEADER_SIZE as u64,
        fmp_timestamp_bytes: 4,
        aead_tag_bytes: fips::noise::TAG_SIZE as u64,
        tree_announce,
        encoded_sizes,
        maximum_safe_tree_depth: maximum_safe_tree_depth(),
    };
    println!("{}", serde_json::to_string_pretty(&manifest).unwrap());
}

fn node(value: u128) -> NodeAddr {
    NodeAddr::from_bytes(value.to_be_bytes())
}

fn tree_announce(depth: u32) -> Vec<u8> {
    let entries: Vec<_> = (0..=depth)
        .map(|index| CoordEntry::new(node(u128::from(index) + 1), u64::from(index), 1000))
        .collect();
    let self_node = entries[0].node_addr;
    let parent = entries.get(1).map_or(self_node, |entry| entry.node_addr);
    let coordinate = TreeCoordinate::new(entries).unwrap();
    let declaration = ParentDeclaration::with_signature(self_node, parent, 1, 1000, [1; 64]);
    TreeAnnounce::new(declaration, coordinate).encode().unwrap()
}

fn fmp_frame(message: &[u8]) -> Vec<u8> {
    let inner = prepend_inner_header(0, message);
    assert!(inner.len() <= usize::from(u16::MAX));
    let header = build_established_header(SessionIndex::new(1), 1, 0, inner.len() as u16);
    let ciphertext_and_tag = vec![0_u8; inner.len() + fips::noise::TAG_SIZE];
    build_encrypted(&header, &ciphertext_and_tag)
}

fn maximum_safe_tree_depth() -> u32 {
    let mut depth = 0_u32;
    loop {
        let message_len = 132_usize + 32 * depth as usize;
        let inner_len = 4 + message_len;
        if inner_len > usize::from(u16::MAX) {
            return depth - 1;
        }
        depth += 1;
    }
}

fn source_hashes() -> BTreeMap<&'static str, String> {
    let sources = [
        ("src/noise/mod.rs", include_bytes!("../proto/../noise/mod.rs").as_slice()),
        ("src/proto/bloom/wire.rs", include_bytes!("../proto/bloom/wire.rs").as_slice()),
        ("src/proto/fmp/wire.rs", include_bytes!("../proto/fmp/wire.rs").as_slice()),
        ("src/proto/fsp/wire.rs", include_bytes!("../proto/fsp/wire.rs").as_slice()),
        ("src/proto/lookup/wire.rs", include_bytes!("../proto/lookup/wire.rs").as_slice()),
        ("src/proto/routing/wire.rs", include_bytes!("../proto/routing/wire.rs").as_slice()),
        ("src/proto/stp/wire.rs", include_bytes!("../proto/stp/wire.rs").as_slice()),
    ];
    sources
        .into_iter()
        .map(|(path, bytes)| (path, format!("{:x}", Sha256::digest(bytes))))
        .collect()
}
