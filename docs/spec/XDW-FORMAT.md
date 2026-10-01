# DocuWorks document format (`.xdw`, `.xbd`): working specification

Status: reverse-engineered from 43 public files (DocuWorks 7 to 9 era, generation
7 and 10 headers) and checked against **DocuWorks Viewer Light 10** (Fuji Xerox /
FUJIFILM), which we drive automatically through Wine (`tools/dwview`). Every
statement marked **[viewer]** was tested by writing a file and opening it in the
viewer; **[corpus]** means it holds for every sample file; **[guess]** is a
working assumption.

All numbers are little-endian unless said otherwise. "1/100 mm" is the unit
of page geometry.

## 1. Elements

The whole file is made of tag-length-value elements:

```
tag      1 byte  (0x60 header, 0x61 segment, 0x63 properties, 0x64 entry, 0x65/0x68 trailer, 0x8n fields)
length   < 0x80: the length itself
         0x80|k: k big-endian bytes follow (k = 1-4)
value
```

DocuWorks writes lengths `0x80`-`0xFE` as `81 nn` and **from `0xFF` on in the
two-byte form** (`82 00 ff`), even where one byte would do [corpus]. Writing
them the same way keeps saved files byte-identical where nothing changed.

Numbers in fields are **signed, big-endian, shortest two's complement** (ASN.1
INTEGER style): `00 c0 13` is 49171, `a1 b1 ae` is the 32-bit value `ffa1b1ae`
[corpus].

## 2. File layout

```
0x60 header   { 0x82 generation (7 or 10), 0x80 …, 0x83 … }
0x61 segment  { 0x64 entry …, 0x63 properties, trailer }
0x61 segment  (every save appends one)
…
```

A save never rewrites earlier bytes. It appends a segment holding the entries
that are new, a complete new properties block, and a trailer. The **last
trailer is the state of the document**; earlier ones are history.

The trailer is found **from the end**: the last four bytes (the value of the
trailer's own field `0x86`) give the length of the trailer's value, counted back
from the end of the file [viewer: changing them makes the file unreadable].
Generation 7 files tag the trailer `0x65`, generation 10 files `0x68`.
Some writers record a segment length 2 bytes short of its trailer; readers
should not rely on segment lengths [corpus: okinawa-pref_79].

### 2.1 Trailer fields

| field | meaning |
|---|---|
| 0x80 | number of entries in effect |
| 0x81 | absolute offsets of those entries, u32 LE each (entry *n* = the *n*-th offset) |
| 0x8d | (index u32, check u32) pairs for picture entries stored raw (see 3.2); optional |
| 0x82 | unknown; kept as is |
| 0x83 | size of the properties block when expanded |
| 0x84 | stored size of the properties block |
| 0x85 | **check value of the stored properties block** (see 2.2) |
| 0x86 | length of the trailer's value (u32 LE), this field included |

The properties block (0x63) sits immediately before the trailer.

### 2.2 Check values

```
check(block) = XOR of every whole 4-byte little-endian word of the block
               (bytes after the last whole word are not counted)
```

- trailer 0x85 = check(stored properties block). **The viewer refuses the file
  ("The file is not a DocuWorks document file.") when it is wrong** [viewer]
- entry 0x81 = check(the entry's 0x82 body value). Not checked when opening
  [viewer], but holds in every sample [corpus]

## 3. Entries (0x64)

```
0x64 { 0x81 check, 0x82 body }
```

### 3.1 Field bodies

When the body starts with `0x80` it is a list of fields:

| field | meaning |
|---|---|
| 0x80 | kind: **4** EMF drawing, **1** WMF drawing, **7** thumbnail (DIB) |
| 0x81 | size of the data once expanded (kind 7: something else) |
| 0x83 | 8 in WMF entries |
| 0x84, 0x85 | width, height (1/100 mm) |
| 0x8d | 3 for page drawings, 1 for annotation drawings |
| 0x90, 0x91 | width, height in drawing pixels |
| 0x8a | 1 = data is LZH-compressed |
| 0x89 | stored size of the data |
| 0x86 | the data |

Compression is LHA **-lh5-** (LZSS, 8 KiB window, static Huffman blocks),
with no LHA archive header. The same codec compresses the properties block.

Drawings stored *inside the properties* (annotations, some pages) use the same
fields but omit 0x8a; they are compressed when 0x81 is larger than the data.

### 3.2 Raw bodies

Otherwise the body is raw data:

- a **picture**: 16-byte header (u32 size, u32 0, u32 width px, u32 height px)
  then a JPEG stream; listed again in trailer 0x8d [corpus]
- an attached **original file** (e.g. the `.docx` the document was printed
  from), LZH-compressed; see 4.4
- a list of attribute definitions that was too large for its record (4.2)

## 4. Properties: the object tree

Expanded, the block is a flat list of records in tree order:

```
62 len {
  80 01 depth
  62 len { kind }          kind as a number
  attribute …
}
```

Attribute tags use the one-byte form (`83` = tag 3, `c7` = private tag 7) or
the long form (`9f 8f 51` = tag 2001). Values of numeric attributes are lists
of *length byte + big-endian two's-complement bytes* (`02 52 08 02 74 04` =
21000, 29700). Text values are stored as they are (Shift_JIS, or UTF-16LE in
attributes whose name ends in `(w`).

### 4.1 Record kinds

```
[0] c013  document root            (binder .xbd: c014 → 1401 → 1402 → c013 …)
  [1] 1303  pages                  lastmid = highest page number
    [2] 1301  page                 5 = size, 3 = number, lastmid
      [3] 1302  placement          52 = position, childdim = size
        [4] 8010  page content     7 = drawing, 58 = thumbnail, 61 = rotation,
                                   300 = picture count, 301… = pictures
      [3] 1302  placement
        [4] 80xx  annotation       5 = size, 3 = number, 7 = drawing
  [1] 1304
  [1] 1306  original files         1000: nd (file entry), nf (name), ut (time)
  [1] 1305
```

| kind | what |
|---|---|
| 8010 | page content |
| 8011 | text annotation |
| 801a | sticky note (付箋); also the kind of the placements inside it |
| 801b | marker (not in the samples) |
| 802e | page form (header / footer) |
| 8033 | date stamp (日付印) |
| 803c | line |
| 803d | rectangle |
| 803e | ellipse |
| 803f | picture annotation |
| 8040 | received stamp (not in the samples) |
| 8042 | polygon (not in the samples) |
| 8045 | custom annotation (shapes of newer versions: polygon points in `%annotation_customdata`) |
| 800f | embedded OLE object |
| c02f | link |
| 8043 | signature (電子印鑑 / PKI): placed like an annotation, not one for the API |

The numbers are the annotation type ids of DocuWorks' published API
(`XDW_AID_…`). The viewer draws every kind from its stored drawing (attribute
7), including kinds it has no settings for [viewer: 8033 and 801a with only
3, 5, 7 show their drawing].

A blank page is a page record with no placement: 5, 3 and lastmid only
[samples; viewer]. New pages and objects take the next number from their
parent's `lastmid` (page list for pages, page for objects, binder list 1401
for binder documents) and update it.

#### Binders

```
[0] c014  binder           4 = binder name (UTF-16LE + one zero byte), %bindersize, %bindercolor
  [1] 1401  documents      lastmid
    [2] 1402               (no attributes)
      [3] c013  document   4 = document name (Shift_JIS + zero), 3 = number;
                           DocuWorks 10 writes the name as 70 (UTF-16 + zero) instead
        [4] 1303 …         the document's pages, as in a .xdw
  [1] 1304
```

A standalone `.xdw` root (c013) usually has no attributes; adding it to a
binder is: copy its subtree three levels deeper under a new 1402, give it 4
and 3, copy the entries it refers to [viewer: added, renamed and reordered
documents show in the document list with their names; Japanese names in
Shift_JIS display correctly. DocuWorks 10 API: reads 4 when there is no 70,
and 70 when there is].

Every object sits in a placement (1302) that gives its position (52, 1/100 mm
from the page's top-left) and size (childdim) [viewer: moving 52 moves the
annotation].

### 4.2 Named attributes

Tags from 2001 up are named per record by **attribute 51**, a list of five-item
definitions: *(tag, type, index, name length, name)*. Types seen: 2 number,
4 text/bytes, 107 size pair. When the list is large it is moved into a raw entry
and attribute 51 holds a reference (private class: `81` entry, `82` length).

### 4.3 Drawings and references

Attribute **7** is an object's drawing:

- private class (`c7`): a reference `81 entry-number 82 body-length`
- context class (`87`/`a7`): the entry body itself (fields of 3.1), inline

**The viewer shows the stored drawing; it does not redraw annotations from
their settings** [viewer: changing `%Text(w` alone changes nothing; removing
7 hides the annotation]. A writer that adds or changes an annotation must
also write its drawing.

Pictures a page drawing uses are listed by attribute **300** (count, little-
endian bytes) and **301…400** (references, in order); from the 101st on they
are attributes named `#pd` whose definition's *index* is the picture number.

### 4.4 Attribute 61: rotation

`61 = 90` turns the object's drawing **clockwise** by 90° inside its box
(270 counter-clockwise, 180) [viewer]. The box (childdim and the object's size
5) is the rotated size. It applies to annotations too. Turning a page is:
swap the page size, move and swap every placement, add 90 to every object's 61.

### 4.5 Annotation settings (as DocuWorks writes them)

- text (8011): `%Text(w` (UTF-16) / `%Text` (Shift_JIS), `%Size` (1/10 pt),
  `%Color` (COLORREF), `%Face`, `%Style`, margins 170, `%ATTR_BKGND_COLOR`
  (65793 = none), `%FrameOnOff` …
- rectangle (803d) / ellipse (803e): `RECTATT_` / `ARCATT_` LINE, THICK,
  LINECOLOR, DRAW, RECTCOLOR, `ATTR_FRAMETRANSPARENT`, `ATTR_FILLTRANSPARENT`
- line (803c): `LINE_COLOR`, `LINE_WIDTH`, `LINE_STYLE`, `ARROW_SORT`,
  `LINE_DATA` (i32 LE point pairs)
- picture (803f): only 57, 5, 61, 3 and 7; the drawing is kind 7 (below).
  **Shown opaque**: white pixels cover the page [viewer: `ATTR_TRANSPARENT`,
  `%ATTR_TRANSPARENT`, `ATTR_FILLTRANSPARENT`, `%Transparent` = 1 change
  nothing]
- text with a frame: `%FrameOnOff`; the viewer program also knows
  `%FrameColor` and `%FrameThick` (names found in DWVLT.exe)
- date stamp (8033), as DocuWorks 10 writes it [made with its API, read back]:
  `STAMPATT_COLOR` (COLORREF), `STAMPATT_TRANSPARENT`, `STAMPATT_POST`
  (upper text, Shift_JIS) with `STAMPATT_POST(w` (UTF-16) and
  `%CCP_STAMPATT_POST` = 932, `STAMPATT_NAME` (lower text, same three),
  `STAMPATT_DATEFLAG` (1 = the stored date), `STAMPATT_ERA`,
  `STAMPATT_BASEYEAR`, `STAMPATT_PREFIX` (the character before the year,
  `'`), `STAMPATT_DATEFORMAT` (`yy.mm.dd`), `STAMPATT_DATEORDER`,
  `STAMPATT_YEAR` / `_MONTH` / `_DAY` (text: a four-digit year with no prefix
  gives `2026.10.01`), 68 = 0, and an inline drawing. The API names
  (`%TopField` …) map onto these
- sticky note (801a): `FSN_COLOR` (COLORREF), `%AutoResize`, 3, `lastmid`, a
  drawing (shadow 0.5 mm in 0x999999, the note with a 0x666666 edge, and the
  text on it); its placement (1302) also has 54 = 1. The text is a child:
  a placement **of kind 801a** (52 relative to the note, childdim) holding a
  text annotation (8011). The API sees one annotation with one child

## 5. Drawings

### 5.1 Page content

Kind 4 is an **EMF**, kind 1 a **WMF**. Page EMFs map their frame
(`rclFrame`, 1/100 mm) onto the box; device units come from `szlDevice` /
`szlMillimeters` (600 dpi in practice).

**Annotation drawings set their window** (SETWINDOWEXTEX, 300 units per inch)
and the viewer fits that window to the annotation box; their frame is not
reliable.

### 5.2 DocuWorks comment records

The DocuWorks printer driver writes most shapes and pictures as GDI comment
records starting with `DW`, played with the current GDI state (brush, pen,
fill mode, transform). Worked out by drawing test pages in the viewer
[viewer]:

```
DW02            begin a path
DW03            clip to the path (replaces the clip)
DW04            fill the path with the brush
DW05            stroke the path with the pen
DW06 l t r b    clip to a rectangle (i32, logical units; replaces the clip)
DW01            (no visible effect)

"DW" op enc     points: u32 count, first point as two i16, then
  enc 0x20      two i16 per point
  enc 0x40      two i8 steps per point
  enc 0x80      one byte per point: x step in the high nibble, y step in the
                low nibble (signed); a nibble of 8 means the step is in the next
                byte (i8), x's byte before y's

  op bit 0x01   the first point starts a new figure (move), the rest are lines
  op bit 0x02   close the figure after these points
  op bit 0x10   the points after the start are Bézier triples
  op 0x20       a polygon on its own, filled (brush only)
  op 0x40       a polyline on its own, stroked

DWa             a picture stored in the record: offsets (from the record start)
                to a BITMAPINFO and bits, as in STRETCHDIBITS
DWb             the next picture of the page's picture list (attribute 301…)
DWc             draw the current picture: bounds, xDest, yDest, xSrc, ySrc,
                cxSrc, cySrc, usage, rop, cxDest, cyDest (i32), as in
                STRETCHDIBITS without the offsets
```

`R2_MASKPEN` (SETROP2 9) works as a highlighter: the colour multiplies with
the page [viewer].

### 5.3 Pictures stored as DIBs (kind 7)

Thumbnails (attribute 58, about 104 px wide, 8-bit with a 216-colour palette)
and picture annotations use kind 7 bodies: 0x81 = the BITMAPINFO's length
(where the bits start), 0x84 / 0x85 = the picture's size in 1/100 mm, 0x86 =

```
BITMAPINFO, then  u32 1, u32 stored size, u32 size, u32 rows,
                  bits compressed with LHA -lh5-
```

[viewer: a picture annotation whose bits are stored plainly shows black].

### 5.4 Picture pages

Kind 5 (DocuWorks 10 scans, photos): the body's data is a JPEG; 0x87 / 0x88
are its size in pixels, 0x8b / 0x8c pixels per metre; the content record
names `cmp` = 5, `dpt` (8 grey, 24 colour), `dpi`.


Scanned and image pages are page EMFs at 600 dpi without a window, that draw
JPEG pictures with `DWb` + `DWc` (the scanner writes strips of 24 rows). Each
picture is a raw entry: `u32 total length, 0, width, height` + JPEG, listed in
attribute 301… and in trailer 0x8d [viewer: a page made this way from one
JPEG shows the picture].

## 6. Writing (what EZPZ File XDW does)

1. Keep the file; append `0x61 { new entries, 0x63 properties, trailer }`.
2. Entry table = the old one, new entries at the end (numbers stay valid).
3. Properties = the edited tree, LZH-compressed; 0x85 = its check value.
4. Trailer 0x86 = the trailer value's length; lengths written the DocuWorks way.
5. Read the result back and compare the tree before handing it out.

Files written this way open in DocuWorks 10 itself (its API reads the same
pages, annotations and settings and draws every page; Desk lists them with
their pages) and in DocuWorks Viewer Light, with added text,
highlighter, rectangle, ellipse and line annotations, moved annotations,
turned, deleted and reordered pages, sticky notes, date stamps, picture
annotations, new blank / picture pages, pages copied from other documents,
and edited binders ([experiments/results.md](../../experiments/results.md)).
Copying a page copies every entry its records refer to (drawing, thumbnail,
pictures, outsourced definitions) and renumbers the references.

## 7. Open questions

- trailer 0x82, header 0x80 / 0x83, page attributes 57 / 62 / 69 / 14
- the WMF comment records `DW\x02\x00…03` in old pages (not needed to draw)
- 8045 custom annotation data, OLE objects
- markers, polygons, received stamps as Desk writes them
- making picture annotations see-through
- signatures (8043: `%sigver`, `%spd`, `%pdbv` …), passwords
