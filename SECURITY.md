# Security

Please report security problems privately to **hello@ezpzfile.com** (Japanese, English or Korean).
Include the commit or version, what happens, and a sample file if you can. We reply within a few days,
keep the report private until a fix is released, and credit you in the release notes if you wish.

脆弱性やクラッシュの報告は **hello@ezpzfile.com** へ非公開でお送りください（日本語・英語・韓国語）。
修正を公開するまで内容は非公開とし、ご希望があればリリースノートでお名前を記載します。

The engine treats every `.xdw` / `.xbd` as untrusted input; see "Opening untrusted files" in
[README.en.md](README.en.md) for the limits and the Web Worker helper.

Text that a black box or an opaque picture covers is left out of extracted text and of the PDF text
layer by default (0.3.0). Note that the `.xdw` file itself still holds the covered text: a document
saved as `.xdw` keeps everything under its annotations, as DocuWorks does. To share a redacted
document, save it as PDF.
