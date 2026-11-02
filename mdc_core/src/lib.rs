#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod model;
pub mod contract;
pub mod identity;
pub mod provenance;
pub mod canonical;

#[cfg(test)]
mod tests {
    use crate::model::observation::{Observation, Source};
    use crate::identity::IdentityAdapter;
    use crate::canonical::snapshot::snapshot;
    use crate::model::relation::Relation;
    use crate::model::composition::Composition;
    use crate::model::state::State;
    use crate::model::transition::Transition;
    use crate::contract::evaluation::LifecycleStatus;
    use crate::identity::CanonicalSemanticBasis;

    // Создаем простой источник для теста
    struct MockSource {
        id: String,
    }

    impl Source for MockSource {
        fn id(&self) -> &str {
            &self.id
        }
    }

    #[test]
    fn test_deterministic_identity() {
        let obs1 = Observation::new(MockSource { id: "BTC".to_string() }, 100, vec![1, 2, 3]).unwrap();
        let obs2 = Observation::new(MockSource { id: "BTC".to_string() }, 100, vec![1, 2, 3]).unwrap();

        let id1 = IdentityAdapter::derive_identity(&obs1);
        let id2 = IdentityAdapter::derive_identity(&obs2);
        
        assert_eq!(id1, id2, "Одинаковые наблюдения должны иметь одинаковый identity");
    }

    #[test]
    fn test_distinct_identity() {
        let obs1 = Observation::new(MockSource { id: "BTC".to_string() }, 100, vec![1, 2, 3]).unwrap();
        let obs2 = Observation::new(MockSource { id: "ETH".to_string() }, 100, vec![1, 2, 3]).unwrap();

        let id1 = IdentityAdapter::derive_identity(&obs1);
        let id2 = IdentityAdapter::derive_identity(&obs2);
        
        assert_ne!(id1, id2, "Разные источники должны давать разные identity");
    }

    #[test]
    fn test_snapshot_preserves_identity() {
        let obs1 = Observation::new(MockSource { id: "BTC".to_string() }, 100, vec![1, 2, 3]).unwrap();
        let snap = snapshot(&obs1);

        let obs2 = Observation::new(MockSource { id: "BTC".to_string() }, 100, vec![1, 2, 3]).unwrap();

        assert_eq!(snap.identity, IdentityAdapter::derive_identity(&obs2));
    }

    #[test]
    fn test_semantic_snapshot_preserves_content() {
        let obs1 = Observation::new(MockSource { id: "BTC".to_string() }, 100, vec![1, 2, 3]).unwrap();
        let snap = snapshot(&obs1);

        let obs2 = Observation::new(MockSource { id: "BTC".to_string() }, 100, vec![1, 2, 3]).unwrap();

        // Проверяем, что семантическое содержание сохранено (это важно для Snapshot!)
        assert_eq!(snap.semantic_basis, obs2.canonical_basis().to_vec());
    }

    #[test]
    fn test_positive_chain() {
        let obs = Observation::new(MockSource { id: "BTC".to_string() }, 100, vec![10, 20, 30]).unwrap();
        let _obs_id = IdentityAdapter::derive_identity(&obs);

        let _measurement = 1;

        let _relation = Relation {
            id: 2,
            kind: "PRECEDES",
            left_ref: 1,
            right_ref: 2,
        };

        let composition = Composition {
            id: 3,
            members: vec![1, 2],
            relations: vec![2],
            constraints_satisfied: true,
            status: LifecycleStatus::Candidate,
        };

        let state = State {
            id: 4,
            source_composition: 3,
            criteria_met: true,
            status: LifecycleStatus::Candidate,
        };

        let transition = Transition {
            id: 5,
            source_state: 4,
            target_state: 6,
            condition_met: true,
            status: LifecycleStatus::Candidate,
        };

        // Проверки целостности
        assert_eq!(composition.members.len(), 2);
        assert_eq!(state.source_composition, composition.id);
        assert_eq!(transition.source_state, state.id);
    }

    #[test]
    fn test_invalid_source_returns_error() {
        // Пытаемся создать Observation с пустым ID
        let result = Observation::new(MockSource { id: "".to_string() }, 100, vec![1, 2, 3]);
        
        // Мы ДОЛЖНЫ получить ошибку, а не успех
        assert!(result.is_err());
    }
}