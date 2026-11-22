//! 一章:它的名字、它的段落。
//!
//! The page carries both in what it sends: the name in the page's title and in
//! a nav element, the paragraphs in a div of the site's reader, one `<p>` a
//! paragraph. No script assembles them, so reading the text is all a chapter
//! needs.

use crate::decode;
use anyhow::{Result, bail};

/// One chapter, read off its page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Chapter {
    /// The chapter's own name, as the page's title gives it.
    pub title: String,
    /// The book's name, for a page that says which book it came from.
    pub book: Option<String>,
    /// Its paragraphs, in order, with the site's font taken out.
    pub paragraphs: Vec<String>,
}

impl Chapter {
    /// How many paragraphs the page still hid behind its font, which is what
    /// a table that has fallen behind the site shows.
    pub fn hidden(&self) -> usize {
        self.paragraphs.iter().filter(|p| crate::is_hidden_all(p)).count()
    }
}

/// What the site writes after a page's title, which is not the title.
const AFTER_TITLE: [&str; 2] = ["在线免费阅读", "免费阅读"];

/// Read a chapter off its page.
pub fn parse(html: &str) -> Result<Chapter> {
    let Some((title, book)) = title_of(html) else {
        bail!("the page names itself in no title");
    };
    let Some(body) = reader_body(html) else {
        bail!("the page has no reader in it, so it holds no chapter: one that has to be paid for, or a page the site has changed");
    };
    let mut paragraphs = Vec::new();
    for raw in body.split("</p>") {
        let text = text_of(raw);
        if text.is_empty() {
            continue;
        }
        let (text, _) = decode(&text);
        paragraphs.push(text);
    }
    if paragraphs.is_empty() {
        bail!("the reader of the page holds no paragraph");
    }
    Ok(Chapter { title, book, paragraphs })
}

/// The chapter's name and the book's, from the page's title.
///
/// The site writes `<book>第<n>章 <name>在线免费阅读_番茄小说官网`, which is the
/// name with the book's in front of it and the site's own after. The nav
/// element carries the chapter's name alone, and is preferred where it is
/// there.
fn title_of(html: &str) -> Option<(String, Option<String>)> {
    let page = between(html, "<title>", "</title>").map(text_of)?;
    let nav = between(html, "muye-reader-nav-title\">", "<").map(text_of);
    let mut chapter = page.as_str();
    for after in AFTER_TITLE {
        if let Some(at) = chapter.find(after) {
            chapter = &chapter[..at];
        }
    }
    let chapter = chapter.trim_end_matches("_番茄小说官网").trim();
    let name = nav.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
        // `第1章 殒命`: the space after the number is where the name starts.
        match chapter.find("章 ") {
            Some(at) => chapter[at + "章 ".len()..].trim().to_string(),
            None => chapter.to_string(),
        }
    });
    let book = match chapter.find('第') {
        Some(0) | None => None,
        Some(at) => Some(chapter[..at].trim().to_string()).filter(|b| !b.is_empty()),
    };
    (!name.is_empty()).then_some((name, book))
}

/// The html of the reader's paragraphs, from where its div opens to where that
/// div closes: the divs inside it are counted, so a paragraph that holds one
/// does not end it.
fn reader_body(html: &str) -> Option<&str> {
    let at = html.find("class=\"muye-reader-content")?;
    let open = html[at..].find('>')? + at + 1;
    let rest = &html[open..];
    let mut depth = 1usize;
    let mut walk = 0usize;
    while depth > 0 {
        let next_open = rest[walk..].find("<div").map(|i| i + walk);
        let next_close = rest[walk..].find("</div>").map(|i| i + walk);
        match (next_open, next_close) {
            (None, None) => return None,
            (Some(o), Some(c)) if o < c => {
                depth += 1;
                walk = o + 4;
            }
            (Some(o), None) => {
                depth += 1;
                walk = o + 4;
            }
            (_, Some(c)) => {
                depth -= 1;
                if depth == 0 {
                    return Some(&rest[..c]);
                }
                walk = c + 6;
            }
        }
    }
    None
}

/// What is between two strings, the first of them taken.
fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let at = text.find(open)? + open.len();
    let rest = &text[at..];
    let end = rest.find(close)?;
    Some(&rest[..end])
}

/// The text of a piece of html: its tags gone, its entities as what they
/// name, and its lines trimmed.
pub(crate) fn text_of(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    let mut text = String::with_capacity(out.len());
    let mut rest = out.as_str();
    while let Some(at) = rest.find('&') {
        text.push_str(&rest[..at]);
        let tail = &rest[at..];
        match tail.find(';').filter(|&end| end <= 10) {
            Some(end) => {
                text.push_str(&entity(&tail[..=end]));
                rest = &tail[end + 1..];
            }
            None => {
                text.push('&');
                rest = &tail[1..];
            }
        }
    }
    text.push_str(rest);
    text.lines().map(str::trim).collect::<Vec<_>>().join(" ").trim().to_string()
}

/// What an entity names: the five the site writes, the space, and the two
/// numbered forms. One it does not name is kept as it was written.
fn entity(entity: &str) -> String {
    match entity {
        "&amp;" => "&".to_string(),
        "&lt;" => "<".to_string(),
        "&gt;" => ">".to_string(),
        "&quot;" => "\"".to_string(),
        "&apos;" | "&#39;" => "'".to_string(),
        "&nbsp;" => " ".to_string(),
        _ => {
            let body = entity.trim_start_matches('&').trim_end_matches(';');
            let number = match body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => body.strip_prefix('#').and_then(|d| d.parse().ok()),
            };
            match number.and_then(char::from_u32) {
                Some(c) => c.to_string(),
                None => entity.to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = include_str!("../tests/data/chapter.html");

    #[test]
    fn a_chapter_is_its_name_and_its_paragraphs() {
        let chapter = parse(PAGE).unwrap();
        assert_eq!(chapter.title, "殒命");
        assert_eq!(chapter.book.as_deref(), Some("皇后重生要谋反"));
        assert_eq!(chapter.paragraphs[0], "寒风呼啸，漫天飞雪。");
        assert_eq!(chapter.paragraphs[1], "三更已过。");
        assert_eq!(chapter.paragraphs.len(), 3, "a div inside a paragraph is not a paragraph of its own");
        assert_eq!(chapter.hidden(), 0);
    }

    #[test]
    fn the_name_is_the_navigation_one_where_the_page_has_it() {
        let page = "<title>某书第3章 上元节在线免费阅读_番茄小说官网</title>\
                    <div class=\"muye-reader-nav-title\">上元节</div>\
                    <div class=\"muye-reader-content noselect\"><p>正文。</p></div>";
        let chapter = parse(page).unwrap();
        assert_eq!(chapter.title, "上元节");
        assert_eq!(chapter.book.as_deref(), Some("某书"));
    }

    #[test]
    fn a_page_with_no_reader_is_refused() {
        let why = parse("<title>某书第1章 殒命在线免费阅读</title><div>要买</div>").unwrap_err();
        assert!(format!("{why:#}").contains("no reader"), "{why:#}");
    }

    #[test]
    fn entities_are_what_they_name() {
        let page = "<title>a第1章 b在线免费阅读</title>\
                    <div class=\"muye-reader-content\"><p>甲&amp;乙 &lt;丙&gt; &#39;丁&#39; &#x4E2D;&nbsp;戊</p></div>";
        let chapter = parse(page).unwrap();
        assert_eq!(chapter.paragraphs[0], "甲&乙 <丙> '丁' 中 戊");
    }

    #[test]
    fn paragraphs_keep_the_order_the_page_sets_them_in() {
        let page = "<title>书第2章 重生在线免费阅读</title>\
                    <div class=\"muye-reader-content\"><p>一。</p><p>二。</p><p>三。</p></div>";
        let chapter = parse(page).unwrap();
        assert_eq!(chapter.paragraphs, ["一。", "二。", "三。"]);
    }
}
