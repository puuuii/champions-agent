use crate::domain::services::PartyIdentifierService;
use crate::party::identifier::PartyIdentifier;
use crate::party::cutout::SideCropConfig;
use std::collections::HashMap;

pub struct OnnxPartyIdentifier {
    identifier: PartyIdentifier,
    crop_config: HashMap<String, SideCropConfig>,
}

impl OnnxPartyIdentifier {
    pub fn new(identifier: PartyIdentifier, crop_config: HashMap<String, SideCropConfig>) -> Self {
        Self { identifier, crop_config }
    }
}

impl PartyIdentifierService for OnnxPartyIdentifier {
    fn identify(&self, _frame: &[u8]) -> Result<Vec<(String, f32)>, String> {
        // 注: 既存の実装では Mat を受け取っているため、ここのシグネチャも調整が必要になる可能性がある。
        // 一旦、トレイトのシグネチャに合わせて実装を構築する。
        Err("Not fully implemented yet".to_string())
    }
}
