//! 一本书的目录:分卷,每卷里的章节名和它们的地址。
//!
//! The site answers the catalogue as one piece of JSON, which is what its own
//! book page shows a catalogue out of: no page to parse, and a chapter that
//! has to be paid for says so, rather than being found empty.

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// One chapter's place in the catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Entry {
    /// What the chapter's page is addressed by.
    #[serde(rename = "itemId")]
    pub id: String,
    /// The chapter's name, as the catalogue gives it.
    pub title: String,
    /// Whether the site withholds the chapter's text from a reader who has
    /// not paid for it.
    #[serde(rename = "isChapterLock")]
    pub locked: bool,
    /// What the site wants before the chapter is read, in what it counts in;
    /// zero is free.
    #[serde(rename = "needPay", default)]
    pub price: i64,
}

impl Entry {
    /// Whether a reader who has not paid may read it.
    pub fn open(&self) -> bool {
        !self.locked && self.price <= 0
    }
}

/// A volume of the book, and the chapters in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    pub name: String,
    pub chapters: Vec<Entry>,
}

/// A book's whole catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Catalogue {
    pub volumes: Vec<Volume>,
}

impl Catalogue {
    /// Every chapter, in the order the book sets them in.
    pub fn chapters(&self) -> impl Iterator<Item = &Entry> {
        self.volumes.iter().flat_map(|v| v.chapters.iter())
    }

    /// How many chapters the book has.
    pub fn len(&self) -> usize {
        self.volumes.iter().map(|v| v.chapters.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The chapter at `at`, counted over the whole book.
    pub fn at(&self, at: usize) -> Option<&Entry> {
        self.chapters().nth(at)
    }

    /// Where the chapter `id` sits, counted over the whole book.
    pub fn position(&self, id: &str) -> Option<usize> {
        self.chapters().position(|c| c.id == id)
    }

    /// What a page shows of the catalogue, one line a chapter, each with the
    /// volume it is in where the book has more than one.
    pub fn lines(&self) -> Vec<String> {
        let many = self.volumes.len() > 1;
        let mut out = Vec::with_capacity(self.len() + self.volumes.len());
        for volume in &self.volumes {
            if many {
                out.push(volume.name.clone());
            }
            for chapter in &volume.chapters {
                let mark = if chapter.open() { "" } else { " [锁]" };
                out.push(format!("{}{mark}", chapter.title));
            }
        }
        out
    }
}

/// The answer the site gives, as much of it as a catalogue needs.
#[derive(Deserialize)]
struct Answer {
    code: i32,
    #[serde(default)]
    message: String,
    data: Option<Data>,
}

#[derive(Deserialize)]
struct Data {
    #[serde(rename = "chapterListWithVolume", default)]
    volumes: Vec<Vec<Entry>>,
    #[serde(rename = "volumeNameList", default)]
    names: Vec<String>,
}

/// Read the catalogue out of the answer.
pub fn parse(json: &str) -> Result<Catalogue> {
    let answer: Answer = serde_json::from_str(json).context("the answer is not the JSON a catalogue is")?;
    if answer.code != 0 {
        bail!("the site answered code {} ({})", answer.code, answer.message);
    }
    let Some(data) = answer.data else {
        bail!("the answer names no catalogue");
    };
    let volumes = data
        .volumes
        .into_iter()
        .enumerate()
        .map(|(at, chapters)| Volume {
            name: data
                .names
                .get(at)
                .cloned()
                .unwrap_or_else(|| format!("第{}卷", at + 1)),
            chapters,
        })
        .collect();
    Ok(Catalogue { volumes })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWER: &str = include_str!("../tests/data/catalog.json");

    #[test]
    fn a_catalogue_is_its_volumes_and_their_chapters() {
        let catalogue = parse(ANSWER).unwrap();
        assert_eq!(catalogue.volumes.len(), 1);
        assert_eq!(catalogue.volumes[0].name, "第一卷：默认");
        assert_eq!(catalogue.len(), 3);
        assert_eq!(catalogue.at(0).unwrap().title, "第1章 殒命");
        assert_eq!(catalogue.at(0).unwrap().id, "6839851004213592580");
        assert!(catalogue.at(0).unwrap().open(), "the first chapter is free");
        assert!(!catalogue.at(2).unwrap().open(), "the third has to be paid for");
        assert_eq!(catalogue.position("6839851109117329932"), Some(1));
        assert_eq!(catalogue.position("0"), None);
    }

    #[test]
    fn a_chapter_that_has_to_be_paid_for_says_so_in_its_line() {
        let lines = parse(ANSWER).unwrap().lines();
        assert_eq!(lines, ["第1章 殒命", "第2章 重生", "第3章 上元节 [锁]"]);
    }

    #[test]
    fn a_volume_is_named_where_the_book_has_more_than_one() {
        let answer = r#"{"code":0,"message":"success","data":{
            "chapterListWithVolume":[[{"itemId":"1","title":"甲","isChapterLock":false,"needPay":0}],
                                     [{"itemId":"2","title":"乙","isChapterLock":false,"needPay":0}]],
            "volumeNameList":["上卷","下卷"]}}"#;
        let catalogue = parse(answer).unwrap();
        assert_eq!(catalogue.lines(), ["上卷", "甲", "下卷", "乙"]);
    }

    #[test]
    fn an_answer_that_refuses_is_why_there_is_no_catalogue() {
        let why = parse(r#"{"code":-352,"message":"risk control"}"#).unwrap_err();
        assert!(format!("{why:#}").contains("-352"), "{why:#}");
        assert!(parse("not json").is_err());
    }
}
