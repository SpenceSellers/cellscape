use super::Rule;

pub fn rule_id_from_lookup(rule: &Rule) -> String {
    serde_json::to_string(rule).expect("Rule serialization cannot fail")
}

pub fn parse_rule_id(id: &str) -> Option<Rule> {
    serde_json::from_str(id).ok()
}

pub fn rule_string_from_lookup(rule: &Rule) -> String {
    rule_id_from_lookup(rule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::CellSource;

    #[test]
    fn static_rule_round_trips() {
        let json = r#"{"lookup":[0,0,0,1,1,1,1,0],"half_width":1,"num_states":2}"#;
        let rule = parse_rule_id(json).unwrap();
        let serialized = rule_id_from_lookup(&rule);
        let rule2 = parse_rule_id(&serialized).unwrap();
        assert_eq!(rule2.lookup.len(), 8);
        assert!(matches!(rule2.lookup[0], CellSource::Static(0)));
        assert!(matches!(rule2.lookup[3], CellSource::Static(1)));
    }

    #[test]
    fn random_entry_round_trips() {
        let json = r#"{"lookup":[0,0,0,{"weights":[[1,0.5],[0,0.5]]},1,1,1,0],"half_width":1,"num_states":2}"#;
        let rule = parse_rule_id(json).unwrap();
        let serialized = rule_id_from_lookup(&rule);
        let rule2 = parse_rule_id(&serialized).unwrap();
        assert!(matches!(rule2.lookup[3], CellSource::Random { .. }));
        assert!(matches!(rule2.lookup[0], CellSource::Static(0)));
    }

    #[test]
    fn multiple_random_entries() {
        let json = r#"{"lookup":[0,{"weights":[[1,0.8],[0,0.2]]},{"weights":[[0,1.0]]},1,1,1,0,0],"half_width":1,"num_states":2}"#;
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
        let json = r#"{"lookup":[0,0,0,{"weights":[]},1,1,1,0],"half_width":1,"num_states":2}"#;
        assert!(parse_rule_id(json).is_none());
    }
}
