<p align="center">
  <img src="docs/images/ezpz_xdw.svg" alt="EZPZ File XDW" width="96">
</p>
<h1 align="center">EZPZ File XDW</h1>
<p align="center"><b>日本語</b> · <a href="README.en.md">English</a> · <a href="README.ko.md">한국어</a></p>

**DocuWorks（`.xdw`）文書を、Mac・Linux・スマホ・ブラウザで開いて、アノテーションを付けて、そのまま保存。**
富士フイルム（旧富士ゼロックス）DocuWorks のファイルを扱うための、ゼロから作ったオープンソースの
エンジン・エディタ・ファイル形式仕様書です。ファイルは端末の外に送信されません。

状態: **v0.2。`.xdw` / `.xbd` に保存し直せるビューア + エディタ（開発者向けプレビュー）。**
富士フイルムビジネスイノベーション株式会社とは関係ありません。

![XDW エディタの画面](docs/images/editor.png)

<sub>見本の見積書に、日付印・蛍光ペン・付箋・透ける承認印を付けたところ。DocuWorks で開いても同じに見えます。</sub>

## できること

インターネットで公開されている DocuWorks ファイル 43 件（第 7・第 10 世代、`.xdw` と `.xbd`）で確認しています。

- 43 件すべてが開き、どのページも未知の描画レコードなしで描ける
- ページ: EMF・WMF の図形、DocuWorks ドライバ独自の圧縮パス・画像レコード、JPEG の画像断片、
  スキャン・回転ページ、アノテーション
- **`.xdw` への保存**: DocuWorks 自身と同じ方式（末尾に 1 セグメントを追加し、それより前は書き換えない）。
  保存したファイルは **DocuWorks Viewer Light** で開ける
- アノテーション: **テキスト、付箋、日付印、蛍光ペン、四角形、楕円、直線、画像** の追加。移動・サイズ変更・
  内容の変更・削除（DocuWorks で付けた既存のアノテーションも移動・削除できる）。画像は **白い部分を透かして**
  下の文字を見せることもでき、DocuWorks でも同じに見える
- ページ内の **文字の選択・コピー**、**文書内の検索**（Ctrl+F）
- ページ: 左右に回転、削除、並べ替え（ページ一覧でドラッグ）。白紙ページ、画像（JPEG / PNG など）、
  別の `.xdw` / `.xbd` のページ、PDF のページ（画像として。pdf.js のために最初の 1 回だけネット接続が必要）の **挿入**
- **バインダー（`.xbd`）**: ページ一覧に文書ごとに表示。`.xdw` の追加、名前の変更、並べ替え、取り外し
- 書き出し: **PDF**（画面と同じ見た目、文字の検索・コピー可）、テキスト。印刷
- 元に戻す / やり直し

ファイル形式の解析結果（DocuWorks が確認する検査値、ドライバ独自の描画レコードなど）は
[docs/spec/XDW-FORMAT.md](docs/spec/XDW-FORMAT.md)（英語）にあります。

署名された文書は開け、署名を選ぶとその情報（電子印鑑か証明書か、署名モジュール、バージョン）が出ます。
編集すると署名が無効になるため、エディタがその旨を表示します。

パスワードや電子証明書で保護された文書は「保護された文書」と表示し、DocuWorks でセキュリティを解除して
から開くよう案内します（このエディタはパスワードを扱いません）。

まだできないこと: 署名の有効性の確認、新しい署名・パスワードを付けること。
詳しくは [計画](docs/PLAN.ja.md)
（[English](docs/PLAN.en.md)、[한국어](docs/PLAN.ko.md)）を参照。

## エディタ

`web/dist/ezpzxdw-editor.html` は 1 ファイルで完結したエディタです。ダブルクリックで開き、`.xdw` を
ウィンドウにドロップして、アノテーションを付けて保存します（Ctrl+S → `.xdw`）。画面は DocuWorks Viewer に
合わせ（左にページ一覧、上にアノテーションの道具、右にプロパティ）、見た目は EZPZ File のデザインです。

キー: T テキスト、S 付箋、D 日付印、H 蛍光ペン、R 四角形、E 楕円、L 直線、Esc 選択、Delete 削除、
矢印（Shift で 1 cm）、Ctrl+Z / Ctrl+Y、Ctrl+S、Ctrl+Shift+S（PDF・テキストで保存）、Ctrl+P、Ctrl+F（検索）。
開いている文書に PDF や画像をドロップすると、ページとして挿入されます。

画面の言葉は日本語と英語があります。英語版は `web/dist/ezpzxdw-editor.en.html` で、どちらのファイルも
`?lang=en` / `?lang=ja` で切り替えられます。ブラウザですぐ使うなら [ezpzfile.com/ja/xdw-editor](https://ezpzfile.com/ja/xdw-editor)。
ほかのサイトに載せるときは `web/dist/ezpzxdw-editor.embed.html` に `web/pkg/` の `.wasm` の URL を入れて使います。
同じサイトのページの枠（iframe）の中で開くと、`web/editor/host.js` の決まりでそのページとやり取りします。

## 本物の DocuWorks で確認

`tools/dwview` は富士フイルムの無料ビューア DocuWorks Viewer Light を Wine で動かし、ファイルが開けるかを
画面写真つきで報告します。`tools/dwapi` は DocuWorks 10 本体（最新版）の API を呼び出し、
保存したファイルを DocuWorks 自身に読ませて（ページ、アノテーションとその設定）、ページを描かせます。
テスト用に保存したファイルはすべて一致し、描画にも成功しました（公開サンプル 38 件 + DocuWorks 10 のサンプル
5 件、すべての編集機能）。DocuWorks Desk の一覧にもページつきで表示されます。
結果は [experiments/results.md](experiments/results.md)（韓国語）にあります。

## 構成

```
engine/crates/ezpzxdw-core   読み込み、描画（EMF/WMF/DW → 表示リスト）、編集、保存、PDF
engine/crates/ezpzxdw-cli    `ezpzxdw` コマンド: info, tree, pages, render, text, edit, check …
engine/crates/ezpzxdw-wasm   ブラウザ用バインディング
web/                       エディタ（ビルド: web/build.sh → web/dist/ezpzxdw-editor.html）
docs/spec/XDW-FORMAT.md    ファイル形式（英語）
docs/PLAN.{ja,en,ko}.md    計画と進捗（日本語、English、한국어）
tools/dwview               DocuWorks Viewer Light を審判にする（Wine）
tools/dwapi                DocuWorks 10 本体を API 経由で審判にする（Wine）
tools/webshot              ブラウザなしでエディタを操作する（Playwright）
corpus/manifest.tsv        公開サンプルファイルの入手先（ファイル自体は含まない）
```

## ビルド

```
cd engine && cargo test                       # コーパスのテストは EZPZXDW_CORPUS=/path/to/samples を付ける
cargo run -p ezpzxdw-cli -- pages file.xdw
web/build.sh                                  # wasm32 ターゲットと wasm-bindgen-cli 0.2.129 が必要
```

## ライセンス

MIT ライセンス（[LICENSE](LICENSE)）。使用・改変・再配布・商用利用は自由です。コピーや改変版には、
著作権表示「Copyright (c) 2026 EZPZ File (https://ezpzfile.com)」とライセンス文をそのまま残してください。
サービスや製品で使うときに、どこかに「Powered by EZPZ File」と表示していただけるとうれしいです（任意）。
商標とサードパーティの素材については [NOTICE](NOTICE) を参照してください。
