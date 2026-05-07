use crate::domain::services::PartyIdentifierService;

#[derive(Default)]
pub struct OnnxPartyIdentifier {}

impl OnnxPartyIdentifier {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PartyIdentifierService for OnnxPartyIdentifier {
    fn identify(&self, _frame: &[u8]) -> Result<Vec<(String, f32)>, String> {
        Err("Not fully implemented yet".to_string())
    }
}
