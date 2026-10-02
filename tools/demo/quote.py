#!/usr/bin/env python3
"""The page in the README screenshot: a made-up quotation (見積書), A4 at
200 dpi, our own text and fictional company names. Writes quote.jpg and a red
seal, seal.png, with a transparent background.

The screenshot document was made from them like this:

    ezpzxdw edit any.xdw ops.json demo.xdw
      ops.json: [{"op":"image_page","at":0,"file":"quote.jpg"},
                 {"op":"delete_page","page":1}]

then, in the editor: a highlighter over the amount, a date stamp, a sticky
note, and seal.png pasted into the 担当印 box (it goes in see-through, since
it has transparent parts).
"""
from PIL import Image, ImageDraw, ImageFont

DPI = 200
W, H = round(210 / 25.4 * DPI), round(297 / 25.4 * DPI)
mm = lambda v: round(v / 25.4 * DPI)
F = "/usr/share/fonts/opentype/noto/"
def font(name, pt):
    return ImageFont.truetype(F + name, round(pt / 72 * DPI), index=0)
SERIF, SERIF_B = "NotoSerifCJK-Regular.ttc", "NotoSerifCJK-Bold.ttc"
SANS, SANS_B = "NotoSansCJK-Regular.ttc", "NotoSansCJK-Bold.ttc"

im = Image.new("RGB", (W, H), "white")
g = ImageDraw.Draw(im)
INK = (24, 24, 32)

def text(x, y, s, f, anchor="la", fill=INK):
    g.text((x, y), s, font=f, fill=fill, anchor=anchor)

def line(x1, y1, x2, y2, w=2):
    g.line((x1, y1, x2, y2), fill=INK, width=w)

L, R = mm(20), W - mm(20)

# title
text(W // 2, mm(22), "御　見　積　書", font(SERIF_B, 22), "mt")
line(W // 2 - mm(38), mm(34), W // 2 + mm(38), mm(34), 3)
line(W // 2 - mm(38), mm(35.2), W // 2 + mm(38), mm(35.2), 1)

# number and date
small = font(SANS, 9.5)
text(R, mm(42), "見積番号　Q-2026-0142", small, "ra")
text(R, mm(47.5), "発行日　2026年10月2日", small, "ra")

# to
text(L, mm(58), "見本商事株式会社　御中", font(SERIF_B, 15))
line(L, mm(66), L + mm(92), mm(66), 2)
body = font(SERIF, 10.5)
text(L, mm(71), "下記のとおりお見積り申し上げます。", body)

# amount
text(L, mm(84), "御見積金額", font(SERIF_B, 12))
text(L + mm(32), mm(81.5), "¥ 369,600 －", font(SERIF_B, 18))
text(L + mm(80), mm(85), "（税込）", body)
line(L, mm(92), L + mm(100), mm(92), 3)

# from, with a seal box
fx = R - mm(66)
text(fx, mm(56), "サンプル株式会社", font(SERIF_B, 13))
for k, s in enumerate(["〒100-0000 東京都千代田区見本町1-2-3", "TEL 03-0000-0000", "営業部　担当：山田"]):
    text(fx, mm(64) + k * mm(5.2), s, font(SANS, 9))
bx, by, bs = R - mm(22), mm(78), mm(22)
g.rectangle((bx, by, bx + bs, by + bs), outline=INK, width=2)
g.rectangle((bx, by - mm(5.5), bx + bs, by), outline=INK, width=2)
text(bx + bs // 2, by - mm(2.75), "担当印", font(SANS, 8), "mm")

# table
cols = [("No.", 12), ("品名", 74), ("数量", 16), ("単位", 14), ("単価", 27), ("金額", 27)]
rows = [
    ("1", "文書変換ツール　年間ライセンス", "1", "式", "180,000", "180,000"),
    ("2", "導入サポート（2時間）", "2", "回", "30,000", "60,000"),
    ("3", "操作研修（オンライン）", "1", "回", "45,000", "45,000"),
    ("4", "保守サービス（月額）", "6", "か月", "8,500", "51,000"),
    ("", "", "", "", "", ""),
    ("", "", "", "", "", ""),
]
ty, rh = mm(104), mm(10)
xs = [L]
for _, w in cols:
    xs.append(xs[-1] + mm(w))
head_f, cell_f = font(SANS_B, 9.5), font(SERIF, 10)
g.rectangle((xs[0], ty, xs[-1], ty + rh), fill=(232, 237, 247))
for i, (name, _) in enumerate(cols):
    text((xs[i] + xs[i + 1]) // 2, ty + rh // 2, name, head_f, "mm")
for r, row in enumerate(rows):
    y = ty + rh * (r + 1)
    for i, v in enumerate(row):
        if not v:
            continue
        if i == 1:
            text(xs[i] + mm(2.5), y + rh // 2, v, cell_f, "lm")
        elif i in (4, 5):
            text(xs[i + 1] - mm(2.5), y + rh // 2, v, cell_f, "rm")
        else:
            text((xs[i] + xs[i + 1]) // 2, y + rh // 2, v, cell_f, "mm")
n = len(rows) + 1
for r in range(n + 1):
    line(xs[0], ty + rh * r, xs[-1], ty + rh * r, 3 if r in (0, 1, n) else 1)
for x in xs:
    line(x, ty, x, ty + rh * n, 3 if x in (xs[0], xs[-1]) else 1)

# totals
sy = ty + rh * n
for k, (a, b) in enumerate([("小計", "336,000"), ("消費税（10%）", "33,600"), ("合計", "369,600")]):
    y = sy + rh * k
    g.rectangle((xs[4], y, xs[-1], y + rh), outline=INK, width=1 if k < 2 else 3)
    line(xs[5], y, xs[5], y + rh, 1)
    text((xs[4] + xs[5]) // 2, y + rh // 2, a, font(SANS_B if k == 2 else SANS, 9.5), "mm")
    text(xs[-1] - mm(2.5), y + rh // 2, b, font(SERIF_B if k == 2 else SERIF, 10), "rm")

# notes
ny = sy + rh * 3 + mm(10)
text(L, ny, "備考", font(SANS_B, 10))
g.rectangle((L, ny + mm(6), R, ny + mm(34)), outline=INK, width=1)
for k, s in enumerate([
    "・有効期限：発行日より30日間",
    "・お支払い条件：月末締め翌月末払い（銀行振込）",
    "・納品方法：ダウンロード（ライセンスキーをメールでお送りします）",
]):
    text(L + mm(4), ny + mm(11) + k * mm(7), s, body)

text(R, H - mm(16), "1 / 1", small, "ra")
im.save("quote.jpg", quality=90, dpi=(DPI, DPI))

# a round red seal "承認" with a transparent background (anti-aliased edges)
S = 4
big = Image.new("RGBA", (300 * S, 300 * S), (0, 0, 0, 0))
d = ImageDraw.Draw(big)
red = (205, 30, 40, 235)
d.ellipse((12 * S, 12 * S, 288 * S, 288 * S), outline=red, width=14 * S)
d.line((40 * S, 150 * S, 260 * S, 150 * S), fill=red, width=8 * S)
sf = ImageFont.truetype(F + SERIF_B, 92 * S, index=0)
d.text((150 * S, 92 * S), "承", font=sf, fill=red, anchor="mm")
d.text((150 * S, 212 * S), "認", font=sf, fill=red, anchor="mm")
big.resize((300, 300), Image.LANCZOS).save("seal.png")
print(W, H)
