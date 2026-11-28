"""生成拼音输入法的码表：音节 -> 按常用度排好的汉字。

数据来源：
- 读音：pypinyin 的词典（多音字取全部读音，第一个是最常用的）
- 常用度：wordfreq 的中文词频表，取单字在表里的先后来排
字集：GB2312 全部汉字，外加词频表里排在前面的一些扩展字。
输出 resources/pinyin.txt，每行 "音节\t字串"。
"""
from pypinyin import pinyin, Style
from wordfreq import top_n_list

MAX_PER_SYLLABLE = 16
EXTRA = 800

order = top_n_list('zh', 300000)
rank = {}
for i, w in enumerate(order):
    if len(w) == 1 and '\u4e00' <= w <= '\u9fff' and w not in rank:
        rank[w] = i

gb = []
for hi in range(0xB0, 0xF8):
    for lo in range(0xA1, 0xFF):
        try:
            gb.append(bytes([hi, lo]).decode('gb2312'))
        except UnicodeDecodeError:
            pass

extra = [c for c in rank if c not in set(gb)][:EXTRA]
chars = list(dict.fromkeys(gb + extra))

def syllables(ch):
    """这个字的读音，第一个是最常用的，去声调并把 ü 写成 v。"""
    allp = pinyin(ch, style=Style.NORMAL, heteronym=True, errors='ignore')
    if not allp:
        return [], None
    reads = [r.replace('ü', 'v') for r in allp[0]]
    return reads, reads[0]

table = {}
for ch in chars:
    reads, main = syllables(ch)
    r = rank.get(ch, 10**9)
    for rd in reads:
        if not rd.isalpha():
            continue
        # 主读音排在次要读音前面
        table.setdefault(rd, []).append((0 if rd == main else 1, r, ch))

lines = []
for rd in sorted(table):
    seen = set()
    picks = []
    for _, _, ch in sorted(table[rd]):
        if ch in seen:
            continue
        seen.add(ch)
        picks.append(ch)
        if len(picks) == MAX_PER_SYLLABLE:
            break
    lines.append(rd + '\t' + ''.join(picks))

import os

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'resources', 'pinyin.txt')
open(out, 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
print('syllables', len(lines), 'chars', len(chars))
