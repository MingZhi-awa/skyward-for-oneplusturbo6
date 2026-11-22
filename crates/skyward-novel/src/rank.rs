//! 榜单:一个分类里的书,按站点的名次排好。
//!
//! The site's ranking pages are sent whole, like the chapters: a page of ten
//! books, each with its name, its author, whether it is finished, how many are
//! reading it, and the chapter it last took. The names are set in the site's
//! own font like the words of a chapter, so they are read through
//! [`crate::decode`] as well.
//!
//! A page of a ranking is addressed `/rank/<男女频>_<页>_<分类>`, and the
//! ranking's own page lists every category it has.

use crate::chapter::text_of;
use crate::decode;

/// One book on a ranking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Book {
    /// What the book's page is addressed by.
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    /// Whether the book is finished, as its entry says.
    pub finished: bool,
    /// How many are reading it, as the site writes it: `23万`.
    pub readers: Option<String>,
    /// The chapter it last took, and where that chapter is.
    pub last: Option<(String, String)>,
}

impl Book {
    /// What a page shows of the book: its name and, under it, who wrote it and
    /// how it stands.
    pub fn line(&self) -> String {
        let mut line = self.title.clone();
        if let Some(author) = &self.author {
            line.push_str(&format!(" · {author}"));
        }
        if self.finished {
            line.push_str(" · 完结");
        }
        line
    }
}

/// One of the rankings the site has: its name, and what its first page is
/// addressed by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranking {
    pub path: String,
    pub name: String,
}

/// The address of a ranking's page, at `page` counted from one.
pub fn url(path: &str, page: usize) -> String {
    // `/rank/1_2_8` is the first page of one; the page is the middle number.
    let path = path.trim_start_matches("/rank/");
    let parts: Vec<&str> = path.split('_').collect();
    if parts.len() == 3 {
        format!("{}/rank/{}_{}_{}", crate::BASE, parts[0], page.max(1), parts[2])
    } else {
        format!("{}{path}", crate::BASE)
    }
}

/// The rankings the site's own page lists, in the order it lists them.
pub fn rankings(html: &str) -> Vec<Ranking> {
    let mut out: Vec<Ranking> = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("href=\"/rank/") {
        rest = &rest[at + "href=\"/rank/".len()..];
        let Some(end) = rest.find('"') else { break };
        let path = &rest[..end];
        let Some(open) = rest[end..].find('>') else { break };
        let body = &rest[end + open + 1..];
        let Some(close) = body.find("</a>") else { break };
        let name = text_of(&body[..close]);
        // Every category is listed once for each ranking it is in, so the
        // same path twice is one ranking.
        if !name.is_empty() && !out.iter().any(|r| r.path == path) {
            out.push(Ranking { path: path.to_string(), name });
        }
        rest = &body[close..];
    }
    out
}

/// The books on one page of a ranking, in the order the page sets them.
pub fn books(html: &str) -> Vec<Book> {
    let mut out = Vec::new();
    for chunk in html.split("<div class=\"title\">").skip(1) {
        // An entry runs on into its author, its description and how it
        // stands, so it is taken as all of it: what is wanted is near enough
        // to the front to be inside it.
        let end = chunk.find("<div class=\"title\">").unwrap_or(chunk.len());
        let block = &chunk[..end];
        let Some(at) = block.find("href=\"/page/") else { continue };
        let rest = &block[at + "href=\"/page/".len()..];
        let Some(quote) = rest.find('"') else { continue };
        let id = rest[..quote].to_string();
        let Some(open) = rest[quote..].find('>') else { continue };
        let after = &rest[quote + open + 1..];
        let Some(close) = after.find("</a>") else { continue };
        let (title, _) = decode(&text_of(&after[..close]));
        let author = value_after(block, "class=\"author\"");
        let finished = block.contains("book-item-footer-status\">已完结");
        let readers = value_after(block, "book-item-count\">")
            .map(|c| c.trim_start_matches("在读：").to_string())
            .filter(|c| !c.is_empty());
        let last = chapter_after(block);
        out.push(Book { id, title, author: author.filter(|a| !a.is_empty()), finished, readers, last });
    }
    out
}

/// The text inside the first element that starts with `marker`, put back from
/// the site's font.
fn value_after(html: &str, marker: &str) -> Option<String> {
    let at = html.find(marker)? + marker.len();
    let rest = &html[at..];
    let close = rest.find("</").unwrap_or(rest.len());
    let (value, _) = decode(&text_of(&rest[..close]));
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

/// The last chapter an entry names, and where it is.
fn chapter_after(html: &str) -> Option<(String, String)> {
    let at = html.find("href=\"/reader/")?;
    let rest = &html[at + "href=\"/reader/".len()..];
    let end = rest.find('"')?;
    let id = rest[..end].to_string();
    let after = &rest[end..];
    let open = after.find('>')?;
    let body = &after[open + 1..];
    let close = body.find("</a>")?;
    let (text, _) = decode(&text_of(&body[..close]));
    let text = text.trim_start_matches("最近更新：").trim().to_string();
    (!text.is_empty()).then_some((id, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = include_str!("../tests/data/rank.html");

    #[test]
    fn a_book_is_what_its_entry_says() {
        let books = books(PAGE);
        assert_eq!(books.len(), 2);
        let first = &books[0];
        assert_eq!(first.id, "7231424472606051384");
        assert_eq!(first.title, "惹金枝", "the name comes back from the site's font");
        assert_eq!(first.author.as_deref(), Some("空留"));
        assert!(first.finished);
        assert_eq!(first.readers.as_deref(), Some("23万"));
        let (chapter, name) = first.last.as_ref().unwrap();
        assert_eq!(chapter, "7398648201110094361");
        assert_eq!(name, "第550章 终（完结）");
        assert!(!books[1].finished, "the second entry is still going");
        assert_eq!(books[1].title, "莫若凌霄");
    }

    #[test]
    fn a_book_says_who_wrote_it_and_how_it_stands() {
        let books = books(PAGE);
        assert_eq!(books[0].line(), "惹金枝 · 空留 · 完结");
        assert_eq!(books[1].line(), "莫若凌霄 · 某作者");
    }

    #[test]
    fn a_ranking_is_addressed_by_its_category_and_its_page() {
        assert_eq!(url("/rank/1_2_8", 1), "https://fanqienovel.com/rank/1_1_8", "the middle number is the page");
        assert_eq!(url("/rank/1_2_8", 3), "https://fanqienovel.com/rank/1_3_8");
        assert_eq!(url("/rank/0_1_1015", 2), "https://fanqienovel.com/rank/0_2_1015");
        assert_eq!(url("/rank/1_2_8", 0), "https://fanqienovel.com/rank/1_1_8", "no page comes before the first");
    }

    #[test]
    fn the_rankings_are_the_ones_the_page_names() {
        let html = "<a href=\"/rank/1_2_1141\" class=\"x\">西方奇幻</a>\
                    <a href=\"/rank/1_2_1140\">东方仙侠</a>\
                    <a href=\"/rank/1_2_1141\">西方奇幻</a>";
        let list = rankings(html);
        assert_eq!(list.len(), 2, "a ranking listed twice is one ranking");
        assert_eq!(list[0], Ranking { path: "1_2_1141".to_string(), name: "西方奇幻".to_string() });
        assert_eq!(list[1].name, "东方仙侠");
    }
}
