//! Name matching rules (SPEC §6.10, §6.1, §6.6).

use crate::fold_table::FOLDS;

fn fold_char(c: char) -> char {
    if c.is_ascii() {
        return c.to_ascii_lowercase();
    }
    match FOLDS.binary_search_by_key(&c, |&(from, _)| from) {
        Ok(at) => FOLDS[at].1,
        Err(_) => c,
    }
}

/// Unicode simple case folding.
pub fn fold(name: &str) -> String {
    if name.is_ascii() {
        return name.to_ascii_lowercase();
    }
    name.chars().map(fold_char).collect()
}

fn starts_with_words(folded_prefix: &str, folded_name: &str) -> bool {
    let mut words = folded_name.split(' ');
    folded_prefix
        .split(' ')
        .all(|word| words.next() == Some(word))
}

/// Whether `prefix` is `name` with zero or more whole words dropped from the end.
pub(crate) fn is_word_prefix(prefix: &str, name: &str) -> bool {
    starts_with_words(&fold(prefix), &fold(name))
}

/// The fewest leading words of `name` that are a word prefix of no name in `others`.
pub(crate) fn shortest_unique<'a>(name: &'a str, others: &[&str]) -> &'a str {
    let others: Vec<String> = others.iter().map(|other| fold(other)).collect();
    let ends = name
        .match_indices(' ')
        .map(|(at, _)| at)
        .chain([name.len()]);
    for end in ends {
        let candidate = fold(&name[..end]);
        if !others
            .iter()
            .any(|other| starts_with_words(&candidate, other))
        {
            return &name[..end];
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_simply() {
        assert_eq!(fold("Damage Control II"), "damage control ii");
        assert_eq!(fold("ΣΑΣ ς"), "σασ σ");
        assert_eq!(fold("ẞ ß"), "ß ß");
        assert_eq!(fold("İ µ Ꭰ ꭰ"), "İ μ Ꭰ Ꭰ");
    }

    #[test]
    fn fold_table_is_sorted() {
        assert!(FOLDS.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn word_prefixes() {
        assert!(is_word_prefix("decayed", "Decayed Damage Control"));
        assert!(is_word_prefix(
            "Decayed Damage Control",
            "Decayed Damage Control"
        ));
        assert!(!is_word_prefix("Decay", "Decayed Damage Control"));
        assert!(!is_word_prefix(
            "Decayed Damage Control II",
            "Decayed Damage Control"
        ));
    }

    #[test]
    fn shortest_unique_names() {
        let others = ["Gravid Damage Control"];
        assert_eq!(
            shortest_unique("Decayed Damage Control", &others),
            "Decayed"
        );
        assert_eq!(shortest_unique("A B", &["A B C"]), "A B");
        assert_eq!(shortest_unique("A B", &["a b"]), "A B");
    }
}
