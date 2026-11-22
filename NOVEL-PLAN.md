# 在 recovery 里看小说：施工单

写于 2026-11-22。这不是设计草稿，是**下次开工照着做**的清单：每个文件改哪、加什么、为什么。
配套代码：`crates/skyward-novel`（已完成，见「已有的东西」）。
参考材料在工作区 `/workspace/fanqie/`：`pua-map.json`（362 项反查表原始数据）、
`decode.py`（离线解码脚本，换字体后重做表时用）、`NOTES.md`（抓取与搜索的全部实测记录）。

**为什么要留这份**：这一版的取舍很多是踩出来的（字体混淆怎么解、哪个入口是 SSG 哪个是空壳、
搜索为什么通不了），隔几天不做就全忘了，重来一遍要两个小时。下次直接从这里接。

---

## 一、范围

要做：首页一项 → 榜单分类 → 某榜的书单 → 一本书的目录 → 阅读页（按屏分页、左右翻页）。

不做（做过判断的）：

| 不做 | 原因 |
| --- | --- |
| 搜索 | `/api/author/search/search_book/v1` 被网关按签名头拦着，GET 空体、POST `-2 参数有误`；详见 `fanqie/NOTES.md`。另外 recovery 的键盘没有中文输入法，"输入书名"本身也不顺 |
| 书库 / 书架页 | 客户端渲染，SSR 只有空壳，抓不到 |
| 封面图 | 站点封面是 JPEG，要加解码器；文字已足够，先不做 |
| 记住读到哪、书架收藏 | 想做可以照 `crates/skyward/src/saved_wlan.rs` 存到 `persist`，v2 再说 |

---

## 二、已有的东西（`crates/skyward-novel`）

三个模块，17 个测试全过，拿真页面验过：

| 文件 | 接口 | 备注 |
| --- | --- | --- |
| `src/table.rs` | `WORDS: [(u32, char); 362]` | 站点混淆字体的反查表；`GEN.md` 写换了字体怎么重做 |
| `src/lib.rs` | `decode(&str) -> (String, usize)`、`is_hidden_all(&str)`、`BASE`、`USER_AGENT`、`rank_url()`、`book_url()`、`catalog_url()`、`chapter_url()` | `decode` 第二个返回值是「表外字符数」，不是错误 |
| `src/chapter.rs` | `chapter::parse(&str) -> Result<Chapter>`，`Chapter { title, book, paragraphs, hidden() }` | 自己数 `<div>` 层级，段落里有嵌套标签也不会断 |
| `src/catalog.rs` | `catalog::parse(&str) -> Result<Catalogue>`，`Catalogue { volumes, chapters(), at(), position(), lines() }`，`Entry { id, title, locked, price, open() }` | `lines()` 直接能喂列表控件，锁章带 `[锁]` |
| `src/rank.rs` | `rank::rankings(&str)`、`rank::books(&str)`、`rank::url(path, page)`，`Book { id, title, author, finished, readers, last, line() }` | `book.item` 里 `last` 是「最新一章」的 id 和名字，可以直接跳最新章 |

提交：`baf7c5c`（正文与表）、`ae8acaa`（榜单）。

**这个 crate 不该长 UI 依赖**：它只做「拿到 html/JSON → 变成结构」，网络和绘制都在外面。

---

## 三、要改的文件，逐个来

### 1. `crates/skyward-ui/Cargo.toml`

```toml
skyward-novel = { path = "../skyward-novel" }
```

### 2. `crates/skyward-ui/src/app/mod.rs`

`pub enum Screen`（第 85 行起）加四屏，命名照现有习惯（`RestoreBackup`、`FileInfo` 这种名词式）：

```rust
/// The rankings a reader may pick a book from.
Novel,
/// The books on one page of one ranking.
NovelBooks,
/// One book's chapters, to pick one.
NovelChapters,
/// A chapter, set a screen at a time.
NovelReading,
```

同时 `Screen` 的 `fn kind`/`title` 之类的方法（看它旁边怎么写的，`mod.rs` 里 85 行往后一路都是）要
补上这四屏的对应项——**漏一个编译就报，这是好事**。

### 3. `crates/skyward-ui/src/app/state.rs`

`State` 加一个字段（`Default for State` 里也要给初值）：

```rust
/// 小说页读到哪儿了：分类、书单、目录、正文，以及正文排好版之后的第几屏。
pub novel: Novel,
```

`Novel` 自己放在 `app/novel.rs` 里（见第 5 条），`state.rs` 只存一个值。

### 4. `crates/skyward-ui/src/app/act.rs`

`Action` 加：

```rust
/// 点了某个榜单分类，`at` 是 `novel.rankings` 里的下标。
NovelRanking(usize),
/// 点了某本书。
NovelBook(String),
/// 点了某一章，`at` 是 `catalogue.at(at)`。
NovelChapter(usize),
/// 阅读页翻页，`-1` 上一屏，`1` 下一屏。
NovelTurn(i8),
/// 书单翻页，`1` 下一页，`-1` 上一页。
NovelPage(i32),
```

`act.rs` 里 `Toggle`/`Open` 的分发处要把这五个接上。

### 5. `crates/skyward-ui/src/app/novel.rs`（新文件，UI 侧的状态机）

照 `app/backup.rs`、`app/wlan.rs` 的写法（它们都是「状态 + 一堆 `pub(super) fn xxx(&mut self, ...)`」）：

```rust
pub struct Novel {
    /// 榜单分类，抓到一次就够，不用每进一次都抓。
    pub rankings: Option<Vec<skyward_novel::rank::Ranking>>,
    /// 当前分类和它第几页（从 1 数）。
    pub ranking: Option<(usize, usize)>,
    pub books: Vec<skyward_novel::rank::Book>,
    pub catalog: Option<skyward_novel::catalog::Catalogue>,
    /// 当前章的关卡下标，用来算「上一章/下一章」。
    pub chapter_at: Option<usize>,
    pub chapter: Option<skyward_novel::chapter::Chapter>,
    /// 正文排版后的行，和当前第几屏。
    pub lines: Vec<String>,
    pub screen_at: usize,
    /// 正在等的东西，画在页面上让用户知道没死。
    pub waiting: Option<Waiting>,
    /// 上一次失败的原因，原样给用户看。
    pub failed: Option<String>,
}

pub enum Waiting { Rankings, Books, Catalog, Chapter }
```

要写的方法（每个都是纯状态变化，**不发网络、不画东西**，方便测）：

- `fn rankings_arrived(&mut self, list: &str) -> Result<()>`（内部调 `rank::rankings`）
- `fn books_arrived(&mut self, html: &str) -> Result<()>`
- `fn catalog_arrived(&mut self, json: &str) -> Result<()>`
- `fn chapter_arrived(&mut self, html: &str) -> Result<()>`
- `fn set_lines(&mut self, lines: Vec<String>, per_screen: usize)`：把行按屏切，`screen_at` 归零或夹到范围内
- `fn turn(&mut self, by: i8, per_screen: usize) -> bool`：翻屏，返回有没有真的动（到头上就 `false`）
- `fn next_chapter(&self) -> Option<&Entry>` / `fn previous_chapter(&self) -> Option<&Entry>`

### 6. `crates/skyward-ui/src/pages.rs`

- `fn page(cx) -> Frame` 里的 `match` 加四臂：

```rust
Screen::Novel => novel_rankings(cx),
Screen::NovelBooks => novel_books(cx),
Screen::NovelChapters => novel_catalog(cx),
Screen::NovelReading => novel_reading(cx),
```

- `pub fn home(cx) -> Frame`（第 198 行）里加**一项**，位置建议在「安装」那组卡片之后、
  「高级」之前。照现有写法，是往 `rows` 里推一个 `Row::new(title).icon(...).subtitle(note).trail(Trail::Arrow)`，
  再往 `opens` 推 `Some(Action::Open(Screen::Novel))`，最后跟着那一组 `widgets::*` 画出来。
  图标要在 `icons.rs` 里挑一个现成的（`Icon::Book` 之类，没有就用最接近的，别新画一套路径）。
- 四个页面函数都照 `pub fn systems(cx)` / `pub fn restore(cx)` 的结构写：`long_page(cx)` 起头、
  逐行 `widgets::row(...)`、`Hit { rect, action }` 收尾。

**阅读页（`novel_reading`）要点**：

```rust
let face = &f.body;            // 用正文那套字，不要标题字
let px = 32.0;                 // 正文行字号，按 t.m 里现有的比例取
let max_w = t.m.w - t.m.margin * 2.0;
let lines = widgets::wrap(&c, face, paragraph, px, max_w);   // 现成的断行，按 unicode-script 断
let per_screen = ((t.m.h - 上边距 - 下边距) / (px * 1.6)) as usize;
```

- 分页时**先把每段 `wrap` 成行，再整体按 `per_screen` 切**，不要按段落切（否则大段落会丢）
- 段首缩进两格：在 `wrap` 之前给段落前加 `"　　"`（全角空格），或让第一行单独处理
- 翻页手势：左滑→下一屏，右滑→上一屏，音量键上下也给一套（`app/input.rs` 里已有音量键映射）
- 到屏尾且是最后一屏时，再往后翻就跳到下一章（调 `novel.next_chapter()`，没有就提示「已是最新」）

### 7. `crates/skyward-ui/src/app/draw.rs` 或 `fmt.rs`

页面上的副标题、失败提示这些文案函数，照 `fmt::transfer_failed` 的写法加：

```rust
pub fn novel_failed(lang: Lang, why: &str) -> String
pub fn novel_book_note(lang: Lang, book: &skyward_novel::rank::Book) -> String   // 「空留 · 完结 · 在读 23万」
pub fn novel_chapter_note(lang: Lang, at: usize, total: usize) -> String         // 「第 3 章 / 共 966 章」
```

### 8. `crates/skyward-ui/locales/*.toml`（**15 个文件，都要加**）

`build.rs` 拿 `en.toml` 当基准，其它语言缺键会 panic（第 41 行注释就写着「每个语言都对照它」）。
加一节 `[novel]`，键名建议（`zh-Hans` 的措辞直接可抄）：

```toml
[novel]
title = "看小说"
rankings = "排行榜"
books = "书单"
chapters = "目录"
loading = "正在加载"
failed = "没能读到：{why}"
locked = "需要购买"
finished = "完结"
serial = "连载中"
readers = "在读 {n}"
previous_screen = "上一屏"
next_screen = "下一屏"
previous_chapter = "上一章"
next_chapter = "下一章"
last_chapter = "已经是最后一章了"
first_chapter = "已经是第一章了"
font_outdated = "站点的字体换过了，有一批字认不出来"
```

英文那版建议（`en.toml` 是基准，其它语言可以慢一步，但**键必须都在**）：

```toml
[novel]
title = "Read a novel"
rankings = "Rankings"
books = "Books"
chapters = "Chapters"
loading = "Loading"
failed = "Could not read it: {why}"
locked = "Needs buying"
finished = "Finished"
serial = "In progress"
readers = "{n} reading"
...
```

其余 13 个语言可以先用英文占位再逐个翻，`build.rs` 只查键在不在。

### 9. `crates/skyward/src/novel.rs`（新文件，抓取线程）

**照 `crates/skyward/src/wlan.rs` 的 Worker 模式**：它用 `worker::telling()` 拿一对收发箱，
线程循环里收命令、发状态；UI 主循环每帧 `self.wlan.as_ref().map(wlan::Worker::told)` 取（见
`crates/skyward/src/ui.rs:2230`）。不要用 `mpsc`——这套框架里没有，别引入第二种模式。

命令与回报：

```rust
pub enum Asked {
    Rankings,
    Books { path: String, page: usize },
    Catalog { book: String },
    Chapter { item: String },
}
pub enum Told {
    Rankings(Result<String>),     // 原始 HTML，解析在 UI 侧做，方便单测
    Books(Result<String>),
    Catalog(Result<String>),
    Chapter(Result<String>),
}
```

网络用现成的 `skyward_net::http::agent(servers)` + `ureq`（rustls 已配好，见 `skyward-net/src/http.rs`），
请求头带 `skyward_novel::USER_AGENT`（桌面 UA 才拿到可读的那一版）。
抓取要**等 Wi-Fi 拿到地址再发**：`novel.rankings` 那一步失败时，回一句「先连 Wi-Fi」,
不要静默重试，因为 `wlan0` 起来要 60-90 秒（见 `ONEPLUS-TURBO6.md` 的已知问题）。

超时给 20 秒、重试一次（DNS 会抖，实测过 `lf3-aef`/`lf6-awef` 那种 CDN 域名解析偶尔超时，虽然这里
发的是 `fanqienovel.com` 本身）。

### 10. `crates/skyward/src/ui.rs`

- 建 `novel::Worker`（和 `wlan::Worker::start` 一起，Wi-Fi 起来之后再起）
- 主循环里每帧 poll 一次，把 `Told` 转给 `app.novel.xxx_arrived(...)`
- 处理五个新 `Action`：`Effect::...` 或直接 `self.novel_...()`，照 `Effect::FileTransfer` 的接法

---

## 四、UI 里要挡住的几种情况

| 情况 | 页面怎么表现 |
| --- | --- |
| 还没连 Wi-Fi | 一句话说清「先连 Wi-Fi」，不要让用户看见「加载中」转圈 |
| 抓取失败 | 显示 `failed` 的 `{why}`，给一个「重试」按钮（`widgets::button` 有现成写法） |
| 锁章 | 目录里带 `[锁]`（`Catalogue::lines()` 已经带了），点了提示 `locked` |
| 表外字符 | `Chapter::hidden() > 0` 时在正文页顶部挂一句 `font_outdated`，正文照常显示 |
| 榜单一页只有 10 本 | 第 3 页以后返回 0 本（实测：分类只有前 20 名），翻到空就停，不要报错 |

---

## 五、测试要补什么

框架的习惯是「每个行为一个测试」，`cargo test -p skyward-ui` 现在 350 个。按这个写：

`crates/skyward-ui/src/app/novel.rs` 的 `#[cfg(test)] mod tests`：

1. 榜单分类列表画出来是 74 行（用 `rank.html` 的 fixture，从 `crates/skyward-novel/tests/data/` 复制到
   `skyward-ui/src/app/tests/` 或直接 `include_str!` 那一份）
2. 点第 3 个分类 → 进 `Screen::NovelBooks`，`ranking == Some((2, 1))`
3. 书单页点第一本 → 进 `Screen::NovelChapters`，书 id 对
4. 目录页点第 5 章 → 进 `Screen::NovelReading`，`chapter_at == Some(4)`
5. `set_lines` 之后第 0 屏是章节开头，`turn(1)` 一次到第 1 屏
6. 在最后一屏 `turn(1)` → 跳到下一章（或提示最后一章）
7. 在第一屏 `turn(-1)` → 提示第一章 / 退到上一章
8. `failed` 时的页面文案里带原因
9. 正文页最上面挂的「表外字符」提示，`hidden() == 0` 时不出

另外两个**必须过**的既有测试：

- `the_typeface_draws_every_text_the_pages_write`：扫描源码里所有 `texts::` 引用，确认字体画得出来
- `every_language_s_words_on_keys_and_sliders_stay_legible`：确认每种语言在按钮上放得下

---

## 六、验收

```sh
cargo test -p skyward-novel -p skyward-config -p skyward-ui
cargo check --workspace
export CC_aarch64_linux_android=aarch64-linux-gnu-gcc
export AR_aarch64_linux_android=aarch64-linux-gnu-ar
cargo check -p skyward --target aarch64-linux-android      # input/wlan/novel 这些 android-only 才编得到
```

上机前先验三件事（跟番茄无关，是 recovery 自己的老问题）：

1. Wi-Fi 起来之后能不能 `https` 到 `fanqienovel.com`（TLS 证书链、时间对不对）
2. 中文渲染：`MiSansVF` 里这些字有没有，一屏 20 行 × 20 字排版要多久（`widgets::wrap` 是逐字试宽的）
3. `device busy` 那个 wlan0 毛病在不在（见 `ONEPLUS-TURBO6.md`）

---

## 七、风险和已知限制

- **字体表会过期**。站点换字体后满屏还是乱码，`Chapter::hidden()` 立刻不为 0。
  重做表的步骤在 `crates/skyward-novel/GEN.md`，约五分钟。
- **全角 `Ｓ` 那一个码位（U+E496）余量为 0**，全角拉丁字母归一化后分不开，中文正文里基本不出现。
- **搜索做不了**（网关签名），**书库/书架是客户端渲染**，能进书的只有榜单和首页推荐。
- **榜单每分类只有前 20 名**（第 3 页返回 0 本），书的数量有限但够翻。
- **抓的是公开网页**，按站点条款只用来看，别批量下载；一本近千章的书一口气抓完很显眼。
