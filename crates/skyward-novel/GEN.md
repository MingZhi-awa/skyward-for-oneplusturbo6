# 重新生成 `src/table.rs`

`table.rs` 是站点字体里那 362 个隐藏码位到真汉字的反查表，不是手写的。站点换了字体之后
（表现：阅读页满屏还是乱码，`Chapter::hidden()` 不为 0），照下面重做一遍。

## 一、确认字体换了

```sh
curl -s -A "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36" \
  "https://fanqienovel.com/reader/<任意章节id>" | grep -o 'awesome-font/c/[a-z0-9]*\.woff2' | head -1
```

拿到的文件名和 `table.rs` 注释里写的 `dc027189e0ba4cd` 不一样，就是换了。

## 二、重做表

需要 `fonttools` 和 `Pillow`：

```sh
pip install fonttools pillow
```

1. 下载字体：`https://lf3-awef.bytetos.com/obj/awesome-font/c/<名字>.woff2`（`lf6` 是同一个文件）
2. `TTFont(path).flavor = None; save('sub.otf')`
3. 用该字体把每个 cmap 码位渲染成 64px 的图，剪到笔画外框，缩成 16x16 二值图
4. 用参考字体 **Noto Sans CJK SC**（与思源黑体同源）把 0x4E00-0x9FFF 与全角区间共 21140 个字
   渲染成同样的 16x16 图，做参考库
5. 每个码位对参考库算汉明距离，取前 40 名
6. 这 40 名再用 128px 渲染缩成 32x32 复核，取最近的一个
7. 按码位升序写成 `WORDS` 表，注释里更新字体文件名

参考实现见仓库外的 `/workspace/fanqie/decode.py` 与 `NOTES.md`（同一套算法，跑通了三本书四章）。

## 三、验收

- `cargo test -p skyward-novel`：表有序、无重复、码位都在私用区
- 抓一章真页面看 `Chapter::hidden()` 是否为 0
- 抽三段正文读一遍，看是不是通顺中文

## 已知不准的一个

`U+E496` 是全角 `Ｓ`。全角拉丁字母按笔画外框归一化之后彼此几乎一样，余量为 0。
中文正文里不出现，出现也只影响一个字母。
