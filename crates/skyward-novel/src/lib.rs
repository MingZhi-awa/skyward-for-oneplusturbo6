//! 番茄小说网网页版:它的目录、一章的正文,以及把字体藏起来的字换回来。
//!
//! The site serves its pages whole: no script has to run, no cookie has to be
//! carried, and no name has to be signed. A chapter is a page of HTML whose
//! paragraphs are already in it, and the catalogue is one answer of JSON.
//!
//! What it does not serve is the words themselves. The words a reader meets
//! most are written as private-use codepoints, which its own font draws as the
//! words; [`decode`] puts them back, through the table in [`table`].

pub mod catalog;
pub mod chapter;
mod table;

pub use table::WORDS;

/// Where the site is.
pub const BASE: &str = "https://fanqienovel.com";

/// What asks for a page. The site answers a phone's browser with another page
/// than it answers a computer's, and only the computer's was made to be read
/// off.
pub const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// The address of a book's page, which carries its whole catalogue.
pub fn book_url(book: &str) -> String {
    format!("{BASE}/page/{book}")
}

/// The address of the catalogue as JSON, which is what the book's page shows
/// without the rest of what that page carries.
pub fn catalog_url(book: &str) -> String {
    format!("{BASE}/api/reader/directory/detail?bookId={book}")
}

/// The address of one chapter.
pub fn chapter_url(item: &str) -> String {
    format!("{BASE}/reader/{item}")
}

/// The text with the site's own font taken out of it: every private-use
/// codepoint it holds replaced by the word that codepoint is drawn as.
///
/// The count of codepoints that are not in the table comes back with it, and
/// is not an error: it is what a page says when the site has made another
/// font, and a caller may yet show the rest of the page.
pub fn decode(text: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut unknown = 0;
    for c in text.chars() {
        match table::WORDS.binary_search_by_key(&(c as u32), |&(cp, _)| cp) {
            Ok(at) => out.push(table::WORDS[at].1),
            Err(_) => {
                if is_hidden(c) {
                    unknown += 1;
                }
                out.push(c)
            }
        }
    }
    (out, unknown)
}

/// Whether `c` is one of the codepoints the site hides a word behind: the
/// private-use area the font covers, U+E000..=U+F8FF.
pub fn is_hidden(c: char) -> bool {
    ('\u{e000}'..='\u{f8ff}').contains(&c)
}

/// Whether `text` begins with the site's hidden writing, rather than words a
/// reader could read: what a page is when the table has fallen behind it.
pub fn is_hidden_all(text: &str) -> bool {
    let mut seen = 0;
    for c in text.chars() {
        if is_hidden(c) {
            return true;
        }
        if !c.is_whitespace() {
            seen += 1;
        }
        if seen > 24 {
            break;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_names_a_word_once() {
        for pair in table::WORDS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "U+{:04X} comes after U+{:04X}", pair[0].0, pair[1].0);
        }
        assert_eq!(table::WORDS.len(), 362, "the font carries 362 glyphs");
        for &(cp, word) in &table::WORDS {
            assert!((0xe000..=0xf8ff).contains(&cp), "U+{cp:04X} is not where the site hides a word");
            assert!(!is_hidden(word), "U+{cp:04X} was mapped to another hidden codepoint");
        }
    }

    #[test]
    fn a_hidden_codepoint_is_the_word_its_glyph_is_drawn_as() {
        // 寒[U+E516]呼啸，漫[U+E4EB]飞雪。 — the first line of a chapter read
        // off the site, and what it says once the font is taken out.
        let (text, unknown) = decode("寒\u{e516}呼啸，漫\u{e4eb}飞雪。");
        assert_eq!(text, "寒风呼啸，漫天飞雪。");
        assert_eq!(unknown, 0);
    }

    #[test]
    fn a_codepoint_the_table_has_not_got_is_counted_and_kept() {
        let (text, unknown) = decode("好\u{f000}的");
        assert_eq!(text, "好\u{f000}的");
        assert_eq!(unknown, 1, "a codepoint past the table is what another font would say");
    }

    #[test]
    fn words_a_reader_could_read_are_left_alone() {
        let (text, unknown) = decode("这是正常的中文，abc 123。");
        assert_eq!(text, "这是正常的中文，abc 123。");
        assert_eq!(unknown, 0);
        assert!(!is_hidden_all("这是正常的中文"));
        assert!(is_hidden_all("\u{e516}\u{e4eb}"));
        assert!(!is_hidden_all("abc"), "text of no hidden codepoint is not hidden");
    }
}
