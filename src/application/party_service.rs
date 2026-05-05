use crate::domain::services::PartyIdentifierService;
use anyhow::Result;

pub struct PartyOrchestrator<T: PartyIdentifierService> {
    identifier: T,
}

impl<T: PartyIdentifierService> PartyOrchestrator<T> {
    pub fn new(identifier: T) -> Self {
        Self { identifier }
    }

    pub fn identify_party(&self, frame: &[u8]) -> Result<Vec<(String, f32)>, String> {
        self.identifier.identify(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::services::PartyIdentifierService;

    struct MockIdentifier;
    impl PartyIdentifierService for MockIdentifier {
        fn identify(&self, _frame: &[u8]) -> Result<Vec<(String, f32)>, String> {
            Ok(vec![("Charizard".to_string(), 0.95)])
        }
    }

    #[test]
    fn test_party_orchestrator() {
        let identifier = MockIdentifier;
        let orchestrator = PartyOrchestrator::new(identifier);
        let result = orchestrator.identify_party(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap()[0].0, "Charizard");
    }
}
