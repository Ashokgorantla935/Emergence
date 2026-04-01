/// Syllable-based fantasy name generator for beings.
/// Produces 3–8 character names with a mix of 2-syllable and 3-syllable patterns.

const SYLLABLES: &[&str] = &[
    "Ael", "Bor", "Cor", "Dal", "El", "Fen", "Gar", "Hal",
    "Ion", "Jor", "Kal", "Lor", "Myr", "Nor", "Or", "Pal",
    "Ren", "Sol", "Tar", "Uth", "Val", "Wyn", "Xar", "Zel",
    "Ash", "Brin", "Cael", "Dun", "Era", "Fyn", "Gil", "Hel",
    "Ira", "Jar", "Ker", "Lun", "Mar", "Nael", "Orin", "Pyr",
    "Qel", "Rin", "Syl", "Tor", "Una", "Vel", "Wyr", "Yor",
];

/// Generate a fantasy name using 2 or 3 syllables.
/// Names are 3–8 characters, first letter capitalized.
pub fn generate_name(rng: &mut fastrand::Rng) -> String {
    let syllable_count = if rng.u32(0..3) == 0 { 3 } else { 2 };

    let mut name = String::with_capacity(8);
    for i in 0..syllable_count {
        let syl = SYLLABLES[rng.usize(..SYLLABLES.len())];
        // Stop adding syllables if we'd exceed 8 chars
        if name.len() + syl.len() > 8 {
            break;
        }
        if i == 0 {
            // First syllable: capitalize first char, lowercase rest
            let mut chars = syl.chars();
            if let Some(first) = chars.next() {
                for c in first.to_uppercase() {
                    name.push(c);
                }
                for c in chars {
                    name.push(c.to_ascii_lowercase());
                }
            }
        } else {
            // Subsequent syllables: all lowercase for smooth joining
            for c in syl.chars() {
                name.push(c.to_ascii_lowercase());
            }
        }
    }

    // Ensure we have at least something (fallback for edge cases)
    if name.is_empty() {
        name.push_str("Ael");
    }

    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_valid_length() {
        let mut rng = fastrand::Rng::with_seed(42);
        for _ in 0..1000 {
            let name = generate_name(&mut rng);
            assert!(!name.is_empty(), "name must not be empty");
            assert!(name.len() <= 8, "name too long: {name}");
            assert!(name.len() >= 2, "name too short: {name}");
            let first = name.chars().next().unwrap();
            assert!(first.is_uppercase(), "first char must be uppercase: {name}");
        }
    }

    #[test]
    fn names_are_varied() {
        let mut rng = fastrand::Rng::with_seed(99);
        let names: std::collections::HashSet<String> =
            (0..100).map(|_| generate_name(&mut rng)).collect();
        // With 48 syllables and 2-3 combos we expect at least 50% unique in 100 draws
        assert!(names.len() > 50, "too little variety: only {} unique names", names.len());
    }
}
