use std::sync::Arc;
use super::{MixingMode, Rule, SimParameters, SimSetup};

pub fn rule_id_from_lookup(rule: &Rule) -> String {
    serde_json::to_string(rule).expect("Rule serialization cannot fail")
}

pub fn parse_rule_id(id: &str) -> Option<Rule> {
    serde_json::from_str(id).ok()
}

pub fn rule_string_from_lookup(rule: &Rule) -> String {
    rule_id_from_lookup(rule)
}

pub fn params_to_json(params: &SimParameters) -> String {
    serde_json::to_string(params).expect("SimParameters serialization cannot fail")
}

pub fn parse_params_json(s: &str) -> Option<SimParameters> {
    serde_json::from_str(s).ok()
}

pub fn setup_to_json(setup: &SimSetup) -> String {
    serde_json::to_string(setup).expect("SimSetup serialization cannot fail")
}

/// Like setup_to_json but omits mask pixel data — safe to store in a UI text field.
pub fn setup_to_json_display(setup: &SimSetup) -> String {
    if let MixingMode::Masked { .. } = setup.mode {
        let mut stripped = setup.clone();
        stripped.mode = MixingMode::Masked { mask_data: Arc::new(Vec::new()) };
        setup_to_json(&stripped)
    } else {
        setup_to_json(setup)
    }
}

pub fn parse_setup_json(s: &str) -> Option<SimSetup> {
    if let Ok(setup) = serde_json::from_str::<SimSetup>(s) {
        return Some(setup);
    }
    // Back-compat: old single-rule JSON (SimParameters) wraps into a Single setup
    if let Ok(p) = serde_json::from_str::<SimParameters>(s) {
        return Some(SimSetup::single(p));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{CellSource, MixingMode};

    #[test]
    fn static_rule_round_trips() {
        let json = r#"{"l":[0,0,0,1,1,1,1,0],"w":1,"s":2}"#;
        let rule = parse_rule_id(json).unwrap();
        let serialized = rule_id_from_lookup(&rule);
        let rule2 = parse_rule_id(&serialized).unwrap();
        assert_eq!(rule2.lookup.len(), 8);
        assert!(matches!(rule2.lookup[0], CellSource::Static(0)));
        assert!(matches!(rule2.lookup[3], CellSource::Static(1)));
    }

    #[test]
    fn random_entry_round_trips() {
        let json = r#"{"l":[0,0,0,{"weights":[[1,0.5],[0,0.5]]},1,1,1,0],"w":1,"s":2}"#;
        let rule = parse_rule_id(json).unwrap();
        let serialized = rule_id_from_lookup(&rule);
        let rule2 = parse_rule_id(&serialized).unwrap();
        assert!(matches!(rule2.lookup[3], CellSource::Random { .. }));
        assert!(matches!(rule2.lookup[0], CellSource::Static(0)));
    }

    #[test]
    fn multiple_random_entries() {
        let json = r#"{"l":[0,{"weights":[[1,0.8],[0,0.2]]},{"weights":[[0,1.0]]},1,1,1,0,0],"w":1,"s":2}"#;
        let rule = parse_rule_id(json).unwrap();
        assert!(matches!(rule.lookup[0], CellSource::Static(0)));
        assert!(matches!(rule.lookup[1], CellSource::Random { .. }));
        assert!(matches!(rule.lookup[2], CellSource::Random { .. }));
        assert!(matches!(rule.lookup[3], CellSource::Static(1)));
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_rule_id("not json").is_none());
        assert!(parse_rule_id("{}").is_none());
    }

    #[test]
    fn rejects_empty_weights() {
        let json = r#"{"l":[0,0,0,{"weights":[]},1,1,1,0],"w":1,"s":2}"#;
        assert!(parse_rule_id(json).is_none());
    }

    #[test]
    fn setup_round_trips_single() {
        use crate::simulation::random_rule;
        let params = SimParameters {
            rule: random_rule(2, 1, &mut rand::rng()),
            noise: 0.0,
            seed: 42,
        };
        let setup = SimSetup::single(params);
        let json = setup_to_json(&setup);
        let setup2 = parse_setup_json(&json).unwrap();
        assert!(matches!(setup2.mode, MixingMode::Single));
        assert_eq!(setup2.rules.len(), 1);
    }

    #[test]
    fn setup_back_compat_single_params() {
        use crate::simulation::random_rule;
        let params = SimParameters {
            rule: random_rule(2, 1, &mut rand::rng()),
            noise: 0.0,
            seed: 42,
        };
        let json = params_to_json(&params);
        let setup = parse_setup_json(&json).unwrap();
        assert!(matches!(setup.mode, MixingMode::Single));
        assert_eq!(setup.rules.len(), 1);
    }
}
