use super::*;

pub(super) fn flow_json(flow: &Flow, path: &[NodeId], status: &str) -> Value {
    json!({
        "flow_id": flow.id, "source": flow.source, "destination": flow.destination,
        "useful_bytes": flow.useful_payload_bytes,
        "session_action": format!("{:?}", flow.session_action).to_lowercase(),
        "shape": flow.shape,
        "path": path, "status": status
    })
}

pub(super) fn hop_json(
    frame: &RoutedFrame,
    from: NodeId,
    to: NodeId,
    deliveries: Value,
    error: Option<&str>,
) -> Value {
    json!({
        "flow_id": frame.flow.id, "message": "session-data", "from": from, "to": to,
        "hop": frame.hop, "path": frame.path, "frame_bytes": frame.frame_bytes,
        "useful_bytes": frame.flow.useful_payload_bytes, "shape": frame.flow.shape,
        "deliveries": deliveries,
        "rejected": error
    })
}

pub(super) fn rejected_hop_json(
    frame: &RoutedFrame,
    from: NodeId,
    to: NodeId,
    edge: crate::EdgeId,
    link: &LinkConfig,
    error: &str,
) -> Value {
    let mut value = hop_json(frame, from, to, json!([]), Some(error));
    let object = value.as_object_mut().unwrap();
    object.insert("edge".to_owned(), json!(edge));
    object.insert("bandwidth_bps".to_owned(), json!(link.bandwidth_bps));
    object.insert("latency_ns".to_owned(), json!(link.latency_ns));
    object.insert("mtu_bytes".to_owned(), json!(link.mtu_bytes));
    value
}

pub(super) fn delivery_json(delivery: &Delivery) -> Value {
    json!({"deliver_at_ns": delivery.deliver_at_ns, "copy": delivery.copy_ordinal})
}
