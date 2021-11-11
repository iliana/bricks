use itertools::Itertools;
use std::collections::HashMap;
use uuid::Uuid;

#[allow(unstable_name_collisions)]
pub(crate) fn box_names(
    names: &HashMap<Uuid, String>,
    first_initial: bool,
) -> HashMap<Uuid, String> {
    let mut new_names: HashMap<Uuid, String> = HashMap::new();
    let mut counts: HashMap<String, usize> = HashMap::new();

    for (id, name) in names {
        let (rem, last) = split_name(name);
        let new = if first_initial {
            format!(
                "{}, {}",
                last,
                rem.split(' ')
                    .filter_map(|s| s.chars().next())
                    .intersperse(' ')
                    .collect::<String>()
            )
        } else {
            last.into()
        };
        new_names.insert(*id, new.clone());
        *counts.entry(new).or_default() += 1;
    }

    new_names.retain(|_, v: &mut String| counts.get(v.as_str()) == Some(&1));
    if new_names.len() < names.len() {
        let missing = names.iter().filter(|(k, _)| !new_names.contains_key(k));
        let extend = if first_initial {
            missing
                .map(|(k, v)| {
                    let (rem, last) = split_name(v);
                    (*k, format!("{}, {}", last, rem))
                })
                .collect()
        } else {
            box_names(&missing.map(|(k, v)| (*k, v.into())).collect(), true)
        };
        new_names.extend(extend);
    }
    new_names
}

fn split_name(s: &str) -> (&str, &str) {
    for (pos, _) in s.rmatch_indices(' ') {
        let first = &s[..pos];
        let last = &s[pos + 1..];
        if !last.split(' ').all(is_suffix) {
            return (first, last);
        }
    }
    ("", s)
}

fn is_suffix(s: &str) -> bool {
    s.is_empty()
        || s == "Jr."
        || s.chars().all(|c| "IVXLCDM".contains(c))
        || s.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
#[test]
fn test() {
    // cases that definitely exist
    assert_eq!(split_name("Wyatt Mason"), ("Wyatt", "Mason"));
    assert_eq!(split_name("Wyatt Mason XIII"), ("Wyatt", "Mason XIII"));
    assert_eq!(split_name("NaN"), ("", "NaN"));
    assert_eq!(split_name("Liquid Friend V"), ("Liquid", "Friend V"));
    assert_eq!(split_name("Bonito Statter Jr."), ("Bonito", "Statter Jr."));
    assert_eq!(split_name("Bob E. Cagayan"), ("Bob E.", "Cagayan"));
    assert_eq!(split_name("Quin Favors "), ("Quin", "Favors "));
    assert_eq!(
        split_name("Y3hirv Hafgy2738riv"),
        ("Y3hirv", "Hafgy2738riv")
    );
    assert_eq!(split_name("HANDSOME SCARF"), ("HANDSOME", "SCARF"));
    assert_eq!(split_name("Clone 101"), ("", "Clone 101"));

    // cases that hopefully never exist
    assert_eq!(split_name("Bob E. Cagayan II"), ("Bob E.", "Cagayan II"));
    assert_eq!(
        split_name("Bonito Statter Jr. V"),
        ("Bonito", "Statter Jr. V")
    );

    let mut m: HashMap<Uuid, String> = HashMap::new();
    m.insert(
        "524ebfe9-59c8-4db1-8b10-0d5432e80a6c".parse().unwrap(),
        "Bonnie Hashmap".into(),
    );
    m.insert(
        "b29db4bb-fef6-4cbc-9ab9-5eac2a477fd2".parse().unwrap(),
        "Bones Hashmap".into(),
    );
    m.insert(
        "0232a61b-cac3-4948-ac86-7d98f351d387".parse().unwrap(),
        "Zenith Hashmap".into(),
    );
    m.insert(
        "11aa628d-1568-4153-b5e1-c52125fbdbcc".parse().unwrap(),
        "Lady Fiesta".into(),
    );

    let mut names = box_names(&m, false).into_values().collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(
        names,
        ["Fiesta", "Hashmap, Bones", "Hashmap, Bonnie", "Hashmap, Z"]
    );

    let mut names = box_names(&m, true).into_values().collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "Fiesta, L",
            "Hashmap, Bones",
            "Hashmap, Bonnie",
            "Hashmap, Z"
        ]
    );
}
