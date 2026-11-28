//! What the letters spelled on the keyboard may be: the characters of the
//! syllable they spell, the commonest first. The table is built into the
//! recovery, one line to a syllable, its characters after a tab.

use std::collections::HashMap;
use std::sync::OnceLock;

/// The syllables and their characters, in the order they are offered.
const TABLE: &'static str = include_str!("../../../resources/pinyin.txt");

/// How many characters the row above the keys offers at the most.
pub const SHOWN: usize = 8;

fn table() -> &'static HashMap<&'static str, &'static str> {
    static BY_SYLLABLE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    BY_SYLLABLE.get_or_init(|| TABLE.lines().filter_map(|line| line.split_once('\t')).collect())
}

/// The spelling of a syllable and the characters it may be: whether the
/// letters type Chinese, and the letters typed so far that have not been
/// turned into a character yet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ime {
    pub chinese: bool,
    /// The letters spelled since the last character was taken.
    pub composing: String,
}

impl Ime {
    /// The characters of the syllable spelled, the commonest first, as many
    /// as the row offers.
    pub fn cands(&self) -> impl Iterator<Item = char> + '_ {
        let found = table().get(self.composing.as_str()).copied().unwrap_or_default();
        found.chars().take(SHOWN)
    }

    /// Character `i` of the row, if the row reaches that far.
    pub fn cand(&self, i: usize) -> Option<char> {
        self.cands().nth(i)
    }

    /// Spell one more letter; a letter the table cannot start a syllable
    /// with is left for the page to type.
    pub fn type_letter(&mut self, ch: char) -> bool {
        if ch.is_ascii_lowercase() {
            self.composing.push(ch);
            true
        } else {
            false
        }
    }

    /// Take away the last letter spelled.
    pub fn back(&mut self) {
        self.composing.pop();
    }

    /// The spelling is done with: a character was taken from it, or the
    /// keyboard is typing letters of another language.
    pub fn picked(&mut self) {
        self.composing.clear();
    }

    /// Letters are spelled and they make a syllable.
    pub fn spelling(&self) -> bool {
        !self.composing.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_syllable_offers_its_commonest_characters_first() {
        let mut ime = Ime { chinese: true, composing: String::new() };
        assert!(ime.cands().next().is_none(), "nothing spelled, nothing to take");
        for ch in "ni".chars() {
            assert!(ime.type_letter(ch));
        }
        assert_eq!(ime.cand(0), Some('你'), "the commonest character of ni comes first");
        assert!(ime.cands().count() <= SHOWN);
        ime.back();
        assert_eq!(ime.composing, "n");
        ime.picked();
        assert!(!ime.spelling());
    }

    #[test]
    fn what_is_spelled_that_is_no_syllable_offers_nothing() {
        let mut ime = Ime::default();
        for ch in "zw".chars() {
            ime.type_letter(ch);
        }
        assert!(ime.cands().next().is_none());
        assert!(ime.spelling(), "the letters are still spelled, to be taken back");
    }

    #[test]
    fn a_capital_is_no_part_of_a_syllable() {
        let mut ime = Ime::default();
        assert!(!ime.type_letter('N'));
        assert!(!ime.type_letter('1'));
        assert_eq!(ime.composing, "");
    }
}
