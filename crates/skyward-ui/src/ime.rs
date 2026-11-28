//! What the letters spelled on the keyboard may be: the characters of the
//! syllable they spell, the commonest first, a page of them at a time. The
//! table is built into the recovery, one line to a syllable, its characters
//! after a tab.

use std::collections::HashMap;
use std::sync::OnceLock;

/// The syllables and their characters, in the order they are offered.
const TABLE: &'static str = include_str!("../../../resources/pinyin.txt");

/// How many characters the row above the keys offers at a time.
pub const SHOWN: usize = 8;

fn table() -> &'static HashMap<&'static str, &'static str> {
    static BY_SYLLABLE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    BY_SYLLABLE.get_or_init(|| TABLE.lines().filter_map(|line| line.split_once('\t')).collect())
}

/// The spelling of a syllable and the characters it may be: whether the
/// letters type Chinese, the letters typed so far that have not been turned
/// into a character yet, and which page of the characters the row shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ime {
    pub chinese: bool,
    /// The letters spelled since the last character was taken.
    pub composing: String,
    /// The page of characters shown, the first at nought.
    page: usize,
}

impl Ime {
    /// Every character of the syllable spelled, the commonest first.
    fn found(&self) -> &'static str {
        table().get(self.composing.as_str()).copied().unwrap_or_default()
    }

    /// The page of characters the row shows.
    pub fn cands(&self) -> impl Iterator<Item = char> + '_ {
        self.found().chars().skip(self.page * SHOWN).take(SHOWN)
    }

    /// Character `i` of the page, if the page reaches that far.
    pub fn cand(&self, i: usize) -> Option<char> {
        self.cands().nth(i)
    }

    /// How many pages of characters the syllable spelled fills.
    pub fn pages(&self) -> usize {
        self.found().chars().count().div_ceil(SHOWN)
    }

    /// There are characters past the page shown.
    pub fn more(&self) -> bool {
        self.page + 1 < self.pages()
    }

    /// The pages past the one shown, and the first again after the last.
    pub fn turn(&mut self) {
        self.page = match self.more() {
            true => self.page + 1,
            false => 0,
        };
    }

    /// Spell one more letter; a letter the table cannot spell a syllable
    /// with is left for the page to type.
    pub fn type_letter(&mut self, ch: char) -> bool {
        match ch.is_ascii_lowercase() {
            true => {
                self.composing.push(ch);
                self.page = 0;
                true
            }
            false => false,
        }
    }

    /// Take away the last letter spelled.
    pub fn back(&mut self) {
        self.composing.pop();
        self.page = 0;
    }

    /// The spelling is done with: a character was taken from it, or the
    /// keyboard is typing letters of another language.
    pub fn picked(&mut self) {
        self.composing.clear();
        self.page = 0;
    }

    /// Letters are spelled.
    pub fn spelling(&self) -> bool {
        !self.composing.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_syllable_offers_its_commonest_characters_first() {
        let mut ime = Ime { chinese: true, ..Ime::default() };
        assert!(ime.cands().next().is_none(), "nothing spelled, nothing to take");
        for ch in "ni".chars() {
            assert!(ime.type_letter(ch));
        }
        assert_eq!(ime.cand(0), Some('你'), "the commonest character of ni comes first");
        assert_eq!(ime.cands().count(), SHOWN);
        ime.back();
        assert_eq!(ime.composing, "n");
        ime.picked();
        assert!(!ime.spelling());
    }

    #[test]
    fn the_characters_past_the_row_are_on_the_next_page() {
        let mut ime = Ime { chinese: true, ..Ime::default() };
        for ch in "yi".chars() {
            ime.type_letter(ch);
        }
        let first: Vec<char> = ime.cands().collect();
        assert!(ime.pages() > 1, "yi brings more characters than a row holds");
        assert!(ime.more());
        ime.turn();
        let second: Vec<char> = ime.cands().collect();
        assert!(!second.is_empty() && second.iter().all(|ch| !first.contains(ch)), "another page: {second:?}");
        // The pages turn round to the first again after the last.
        let mut ime = Ime { chinese: true, ..Ime::default() };
        ime.type_letter('n');
        ime.type_letter('i');
        let pages = ime.pages();
        for _ in 0..pages {
            ime.turn();
        }
        assert_eq!(ime.cand(0), Some('你'));
        // Spelling again starts from the first page.
        let mut ime = Ime { chinese: true, ..Ime::default() };
        for ch in "yi".chars() {
            ime.type_letter(ch);
        }
        ime.turn();
        ime.back();
        ime.type_letter('i');
        assert_eq!(ime.cand(0), Some('一'), "the page is the first again");
    }

    #[test]
    fn what_is_spelled_that_is_no_syllable_offers_nothing() {
        let mut ime = Ime::default();
        for ch in "zw".chars() {
            ime.type_letter(ch);
        }
        assert!(ime.cands().next().is_none());
        assert_eq!(ime.pages(), 0);
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
