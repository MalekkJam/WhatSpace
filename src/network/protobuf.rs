use super::bundle::{BundleKind, MsgStatus, ProtobufBundle};
use crate::routing::model::{
    Bundle, BundleKind as ModelBundleKind, MsgStatus as ModelMsgStatus, Node,
};
use chrono::DateTime;
use protobuf::Message;
use uuid::Uuid;

impl From<&Bundle> for ProtobufBundle {
    fn from(b: &Bundle) -> Self {
        let (kind, kind_data) = match b.kind.clone() {
            ModelBundleKind::Data { msg } => (BundleKind::DATA, msg),
            ModelBundleKind::Ack { ack_bundle_id } => (BundleKind::ACK, ack_bundle_id.to_string()),
            ModelBundleKind::RequestSV { from } => (BundleKind::REQUESTSV, from.to_string()),
            ModelBundleKind::SummaryVector { ids } => (
                BundleKind::SUMMARYVEC,
                ids.iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        };

        ProtobufBundle {
            id: b.id.to_string(),
            source_id: b.source.id.to_string(),
            source_name: b.source.name.clone(),
            source_address: b.source.address.clone(),
            source_port: b.source.port as u32,
            source_peers: b.source.peers.iter().map(|id| id.to_string()).collect(),
            destination_id: b.destination.id.to_string(),
            destination_name: b.destination.name.clone(),
            destination_address: b.destination.address.clone(),
            destination_port: b.destination.port as u32,
            destination_peers: b
                .destination
                .peers
                .iter()
                .map(|id| id.to_string())
                .collect(),
            timestamp: b.timestamp.timestamp(),
            ttl: b.ttl,
            kind: kind.into(),
            kind_data,
            shipment_status: MsgStatus::PENDING.into(),
            ..Default::default()
        }
    }
}

impl From<ProtobufBundle> for Bundle {
    fn from(p: ProtobufBundle) -> Self {
        Bundle {
            id: Uuid::parse_str(&p.id).unwrap_or_default(),
            source: Node {
                id: Uuid::parse_str(&p.source_id).unwrap_or_default(),
                name: p.source_name,
                address: p.source_address,
                port: p.source_port as u16,
                peers: p
                    .source_peers
                    .iter()
                    .map(|id| Uuid::parse_str(id).unwrap_or_default())
                    .collect(),
                routing_engine: None,
            },
            destination: Node {
                id: Uuid::parse_str(&p.destination_id).unwrap_or_default(),
                name: p.destination_name,
                address: p.destination_address,
                port: p.destination_port as u16,
                peers: p
                    .destination_peers
                    .iter()
                    .map(|id| Uuid::parse_str(id).unwrap_or_default())
                    .collect(),
                routing_engine: None,
            },
            timestamp: DateTime::from_timestamp(p.timestamp, 0).unwrap_or_default(),
            ttl: p.ttl,
            kind: match p.kind.enum_value_or_default() {
                BundleKind::ACK => ModelBundleKind::Ack {
                    ack_bundle_id: Uuid::parse_str(&p.kind_data).unwrap_or_default(),
                },
                _ => ModelBundleKind::Data { msg: p.kind_data },
            },
            shipment_status: ModelMsgStatus::Pending,
        }
    }
}

pub fn serialize(bundle: &ProtobufBundle) -> Option<Vec<u8>> {
    match bundle.write_to_bytes() {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            eprintln!("failed to serialize bundle: {}", e);
            None
        }
    }
}

pub fn deserialize(data: &[u8]) -> Option<ProtobufBundle> {
    match ProtobufBundle::parse_from_bytes(data) {
        Ok(bundle) => Some(bundle),
        Err(e) => {
            eprintln!("failed to deserialize bundle: {}", e);
            None
        }
    }
}
