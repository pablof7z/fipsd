use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn enrich_rejection(
    mut value: Value,
    frame: &RecoveryFrame,
    from: NodeId,
    to: NodeId,
    edge: crate::EdgeId,
    link: &LinkConfig,
    from_transport: &str,
    to_transport: &str,
) -> Value {
    let object = value.as_object_mut().unwrap();
    object.insert("from".to_owned(), json!(from));
    object.insert("to".to_owned(), json!(to));
    object.insert("edge".to_owned(), json!(edge));
    object.insert("hop".to_owned(), json!(frame.hop));
    object.insert("frame_bytes".to_owned(), json!(frame.frame_bytes));
    object.insert("bandwidth_bps".to_owned(), json!(link.bandwidth_bps));
    object.insert("latency_ns".to_owned(), json!(link.latency_ns));
    object.insert("mtu_bytes".to_owned(), json!(link.mtu_bytes));
    object.insert("from_transport".to_owned(), json!(from_transport));
    object.insert("to_transport".to_owned(), json!(to_transport));
    object.insert("deliveries".to_owned(), json!([]));
    value
}
