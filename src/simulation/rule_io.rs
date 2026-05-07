use super::{CellSource, Rule};

fn format_weight(w: f32) -> String {
    let s = format!("{:.4}", w);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn format_cell_source(src: &CellSource) -> String {
    match src {
        CellSource::Static(d) => char::from_digit(*d as u32, 10).unwrap().to_string(),
        CellSource::Random { cumulative, values } => {
            let mut parts = Vec::with_capacity(cumulative.len());
            let mut prev = 0.0f32;
            for (&cum, &val) in cumulative.iter().zip(values.iter()) {
                parts.push(format!("{};{}", val, format_weight(cum - prev)));
                prev = cum;
            }
            format!("{{{}}}", parts.join("|"))
        }
    }
}

fn parse_entries(s: &str, num_states: usize) -> Option<Vec<CellSource>> {
    let mut result = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c == '{' {
            chars.next();
            let mut inner = String::new();
            loop {
                match chars.next()? {
                    '}' => break,
                    ch => inner.push(ch),
                }
            }
            result.push(parse_random_entry(&inner, num_states)?);
        } else if let Some(d) = c.to_digit(10) {
            chars.next();
            if (d as usize) >= num_states { return None; }
            result.push(CellSource::Static(d as u8));
        } else {
            return None;
        }
    }
    Some(result)
}

fn parse_random_entry(s: &str, num_states: usize) -> Option<CellSource> {
    let pairs: Option<Vec<(f32, u8)>> = s.split('|')
        .map(|part| {
            let mut kv = part.splitn(2, ';');
            let state: usize = kv.next()?.trim().parse().ok()?;
            let weight: f32 = kv.next()?.trim().parse().ok()?;
            if state >= num_states || weight < 0.0 { return None; }
            Some((weight, state as u8))
        })
        .collect();
    let pairs = pairs?;
    if pairs.is_empty() { return None; }
    Some(CellSource::random(pairs))
}

pub fn rule_string_from_lookup(rule: &Rule) -> String {
    rule.lookup.iter().map(format_cell_source).collect()
}

pub fn rule_id_from_lookup(rule: &Rule) -> String {
    let rule_width = 2 * rule.half_width + 1;
    let digits = rule_string_from_lookup(rule);
    format!("{};{};{}", rule.num_states, rule_width, digits)
}

pub fn parse_rule_id(id: &str) -> Option<Rule> {
    let mut parts = id.splitn(3, ';');
    let num_states: usize = parts.next()?.parse().ok()?;
    let rule_width: usize = parts.next()?.parse().ok()?;
    if rule_width == 0 || rule_width % 2 == 0 { return None; }
    let half_width = (rule_width - 1) / 2;
    let entries_str = parts.next()?;
    rule_lookup_from_string(entries_str, num_states, half_width)
}

pub fn rule_lookup_from_string(s: &str, num_states: usize, half_width: usize) -> Option<Rule> {
    let width = 2 * half_width + 1;
    let expected_len = num_states.pow(width as u32);
    let lookup = parse_entries(s, num_states)?;
    if lookup.len() != expected_len { return None; }
    Some(Rule::new(lookup, num_states, half_width))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_rule_round_trips() {
        let rule = parse_rule_id("2;3;00011110").unwrap();
        assert_eq!(rule_id_from_lookup(&rule), "2;3;00011110");
    }

    #[test]
    fn random_entry_serializes() {
        let src = CellSource::random(vec![(1.0, 0), (1.0, 1)]);
        let s = format_cell_source(&src);
        assert!(s.starts_with('{') && s.ends_with('}'));
        assert!(s.contains('|'));
    }

    #[test]
    fn random_entry_round_trips() {
        let original = "2;3;000{1;0.5|0;0.5}1110";
        let rule = parse_rule_id(original).unwrap();
        let serialized = rule_id_from_lookup(&rule);
        let rule2 = parse_rule_id(&serialized).unwrap();
        assert_eq!(rule2.lookup.len(), rule.lookup.len());
        assert!(matches!(rule2.lookup[3], CellSource::Random { .. }));
        assert!(matches!(rule2.lookup[0], CellSource::Static(0)));
    }

    #[test]
    fn multiple_random_entries() {
        // k=2 width=3 needs 2^3=8 entries: 1 + 1 + 1 + 5
        let rule = parse_rule_id("2;3;0{1;0.8|0;0.2}{0;1}11100").unwrap();
        assert!(matches!(rule.lookup[0], CellSource::Static(0)));
        assert!(matches!(rule.lookup[1], CellSource::Random { .. }));
        assert!(matches!(rule.lookup[2], CellSource::Random { .. }));
        assert!(matches!(rule.lookup[3], CellSource::Static(1)));
    }

    #[test]
    fn rejects_wrong_entry_count() {
        assert!(parse_rule_id("2;3;000111").is_none()); // 6 instead of 8
        assert!(parse_rule_id("2;3;000111000").is_none()); // 9 instead of 8
    }

    #[test]
    fn rejects_out_of_range_state() {
        assert!(parse_rule_id("2;3;00021110").is_none()); // state 2 invalid for k=2
        assert!(parse_rule_id("2;3;000{3;0.5|0;0.5}110").is_none()); // state 3 invalid
    }

    #[test]
    fn rejects_malformed_random() {
        assert!(parse_rule_id("2;3;000{}1110").is_none()); // empty braces
        assert!(parse_rule_id("2;3;000{1}1110").is_none()); // missing weight
        assert!(parse_rule_id("2;3;000{1;abc}1110").is_none()); // non-numeric weight
    }

    #[test]
    fn weights_need_not_sum_to_one() {
        // unnormalized weights are fine — they get normalized internally
        // k=2 width=3: 3 + 1 + 4 = 8 entries
        let rule = parse_rule_id("2;3;000{1;50|0;50}1100").unwrap();
        assert!(matches!(rule.lookup[3], CellSource::Random { .. }));
    }

    #[test]
    fn format_weight_strips_trailing_zeros() {
        assert_eq!(format_weight(0.5), "0.5");
        assert_eq!(format_weight(1.0), "1");
        assert_eq!(format_weight(0.2500), "0.25");
    }
}
