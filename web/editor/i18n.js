// Words on screen: Japanese (the source) or English.
//
// The language comes from ?lang=en|ja, else from <html lang> (build.sh writes
// one page per language). The Japanese text in app.js and editor.html is the
// key; EN below holds its English. A missing key shows the Japanese as is.
//
//   tr("「{0}」は見つかりません", q)   placeholders {0} {1} … may move in the English
//   translateDom()                     the page's own text, titles and labels

const LANG = (() => {
  let q = null;
  try { q = new URLSearchParams(location.search).get("lang"); } catch {}
  const l = (q || document.documentElement.lang || "ja").toLowerCase();
  return l.startsWith("en") ? "en" : "ja";
})();
document.documentElement.lang = LANG;

const EN = {
  // ---- page
  "XDW エディタ | EZPZ File": "XDW Editor | EZPZ File",
  "XDW エディタ": "XDW Editor",
  "サイトのメニュー": "Site Menu",
  "メニュー": "Menu",
  "未保存の変更": "Unsaved changes",
  "ツールバー": "Toolbar",
  "ページ": "Pages",
  "バインダーに文書を追加": "Add a document to the binder",
  "文書内を検索": "Find in document",
  "前を検索 (Shift+Enter)": "Previous (Shift+Enter)",
  "次を検索 (Enter)": "Next (Enter)",
  "閉じる (Esc)": "Close (Esc)",
  "プロパティ": "Properties",
  "幅に合わせる": "Fit Width",
  "縮小": "Zoom Out",
  "拡大": "Zoom In",
  "DocuWorks 文書をここにドロップ": "Drag and drop a DocuWorks file here",
  "DocuWorks がなくても .xdw と .xbd を開いて、注釈を付けて保存できます。白紙から新しく作ることも、PDF や画像をドロップして DocuWorks 文書にすることもできます。": "Open .xdw and .xbd files without DocuWorks, add annotations and save. You can also start from a blank page, or drop PDFs and pictures to turn them into a DocuWorks file.",
  "ファイルを開く": "Open File",
  "新規文書": "New Document",
  "新規作成": "New",
  "無題": "Untitled",
  "新しい文書を作りました。テキストや付箋を置くか、PDF・画像をドロップしてページにできます。": "New document ready. Add text or sticky notes, or drop PDFs and pictures to add them as pages.",
  "ファイルはこのブラウザの中だけで処理されます": "Your file never leaves this browser",
  "ここにドロップ（.xdw / .xbd は開く・PDF や画像はページとして挿入）": "Drop here (.xdw / .xbd opens; a PDF or picture is inserted as pages)",

  // ---- dialogs
  "名前を付けて保存": "Save As",
  "ファイル名": "File name",
  "DocuWorks 文書 (.xdw)": "DocuWorks document (.xdw)",
  "DocuWorks バインダー (.xbd)": "DocuWorks binder (.xbd)",
  "DocuWorks でそのまま開けます": "Opens in DocuWorks as is",
  "画面と同じ見た目。文字の検索・コピーもできます": "Looks the same as on screen. Text can be searched and copied",
  "テキスト (.txt)": "Text (.txt)",
  "文書の文字だけ": "Only the text of the document",
  "キャンセル": "Cancel",
  "保存": "Save",
  "確認": "Confirm",
  "保存しない": "Don't Save",
  "日付印": "Date stamp",
  "上の文字": "Top text",
  "日付": "Date",
  "下の文字": "Bottom text",
  "例: 受付・部署名": "e.g. Received, department",
  "例: 名前": "e.g. Name",
  "色": "Color",
  "大きさ": "Size",
  "クリックした場所に押します。あとから右のパネルで直せます。": "The stamp goes where you click. You can change it later in the panel on the right.",
  "押す": "Stamp",
  "ページを挿入": "Insert Pages",
  "すべてのページ": "All pages",
  "ページを指定": "Pages",
  "例: 1-3, 5": "e.g. 1-3, 5",
  "挿入": "Insert",
  "入力": "Input",
  "名前": "Name",
  "情報": "Info",
  "閉じる": "Close",

  // ---- menus
  "ファイル": "File",
  "開く…": "Open…",
  "上書き保存 (.xdw)": "Save (.xdw)",
  "名前を付けて保存…": "Save As…",
  "PDF として保存": "Save as PDF",
  "テキストとして保存": "Save as Text",
  "印刷…": "Print…",
  "編集": "Edit",
  "元に戻す": "Undo",
  "やり直し": "Redo",
  "削除": "Delete",
  "文書内を検索…": "Find in Document…",
  "表示": "View",
  "ページ一覧": "Page List",
  "白紙ページを挿入": "Insert Blank Page",
  "ファイルからページを挿入…": "Insert Pages from File…",
  "左へ 90° 回転": "Rotate 90° Left",
  "右へ 90° 回転": "Rotate 90° Right",
  "180° 回転": "Rotate 180°",
  "前へ移動": "Move Up",
  "後ろへ移動": "Move Down",
  "ページを削除": "Delete Page",
  "アノテーション": "Annotations",
  "選択": "Select",
  "テキスト": "Text",
  "蛍光ペン": "Highlighter",
  "四角形": "Rectangle",
  "楕円": "Ellipse",
  "直線": "Line",
  "付箋": "Sticky note",
  "画像を貼る…": "Add Picture…",
  "バインダー": "Binder",
  "文書を追加…": "Add Document…",
  "文書名を変更…": "Rename Document…",
  "文書を上へ": "Move Document Up",
  "文書を下へ": "Move Document Down",
  "文書をバインダーから外す": "Remove Document from Binder",

  // ---- top bar and toolbar
  "開く": "Open",
  "開く ({0}O)": "Open ({0}O)",
  "共有": "Share",
  "DocuWorks 文書として保存 ({0}S)": "Save as DocuWorks ({0}S)",
  "形式を選んで保存": "Save in Another Format",
  "DocuWorks 文書で保存": "Save as DocuWorks document",
  "DocuWorks バインダーで保存": "Save as DocuWorks binder",
  "PDF で保存": "Save as PDF",
  "テキストで保存": "Save as text",
  "元に戻す ({0}Z)": "Undo ({0}Z)",
  "やり直し ({0}Y)": "Redo ({0}Y)",
  "印刷 ({0}P)": "Print ({0}P)",
  "蛍光ペンの色": "Highlighter color",
  "テキストの大きさ": "Text size",
  "線の太さ": "Line width",
  "ページを左へ 90° 回転": "Rotate page 90° left",
  "ページを右へ 90° 回転": "Rotate page 90° right",

  // ---- page list
  "{0} 文書 · {1} ページ": ["{0} doc · {1} pages", "{0} docs · {1} pages"],
  "{0} ページ": ["{0} page", "{0} pages"],
  "左へ90°回転": "Rotate 90° left",
  "右へ90°回転": "Rotate 90° right",
  "文書名を変更": "Rename document",
  "（名前なし）": "(no name)",

  // ---- properties panel
  "文書を開くと、ここに選んだ注釈の設定が出ます。": "Open a document, and the settings of the annotation you select show up here.",
  "ページ数": "Pages",
  "用紙": "Paper",
  "注釈": "Annotations",
  "{0} 個": "{0}",
  "注釈を選ぶ: クリック<br>移動: ドラッグ / 矢印キー<br>大きさ: 角をドラッグ（Shift で縦横比を保つ）<br>テキストの修正: ダブルクリック<br>削除: Delete キー": "Select an annotation: click<br>Move: drag or arrow keys<br>Resize: drag a corner (Shift keeps the shape)<br>Edit text: double-click<br>Delete: Delete key",
  "マーカー": "Marker",
  "画像": "Picture",
  "受信印": "Received stamp",
  "図形": "Shape",
  "多角形": "Polygon",
  "リンク": "Link",
  "署名": "Signature",
  "ページフォーム": "Page form",
  "オブジェクト": "Object",
  "種類": "Type",
  "位置": "Position",
  "なし": "None",
  "文字": "Text",
  "太字": "Bold",
  "サイズ": "Size",
  "背景": "Background",
  "今日の日付にする": "Use today's date",
  "線": "Line",
  "塗り": "Fill",
  "透過": "See-through",
  "白を透かす": "Clear white",
  "透かさない": "Opaque",
  "白い部分から下の文字が見えます。DocuWorks でも同じに見えます（貼り付けた図として保存）。": "Text under the white parts shows through. DocuWorks shows it the same way (saved as a pasted picture).",
  "白い部分も含めて不透明です（DocuWorks の画像注釈）。": "Opaque, white parts included (a DocuWorks picture annotation).",
  "署名の種類": "Signature type",
  "電子印鑑": "Digital seal",
  "証明書（PKI）": "Certificate (PKI)",
  "モジュール": "Module",
  "バージョン": "Version",
  "署名です。ここでは動かしたり消したりできません。編集して保存すると署名は無効になります。有効性の確認は DocuWorks で行ってください。": "This is a signature. It can't be moved or deleted here. Saving after an edit makes the signature invalid. Check whether it is valid in DocuWorks.",
  "この注釈は移動と削除ができます（中身の変更は DocuWorks で）。": "You can move or delete this annotation (change what is in it in DocuWorks).",
  "太さ": "Width",
  "レター": "Letter",
  "{0} 横": "{0} landscape",
  "{0} 縦": "{0} portrait",

  // ---- status bar
  "<b>{0}</b> / <b>{1}</b> ページ": "Page <b>{0}</b> / <b>{1}</b>",

  // ---- messages
  "{0} ページを表示できませんでした": "Couldn't show page {0}",
  "最後の 1 ページは削除できません": "The last page can't be deleted",
  "ページの削除": "Delete Page",
  "{0} ページを削除しますか？（元に戻すことができます）": "Delete page {0}? (You can undo this.)",
  "この文書には署名があります。編集して保存すると、署名は無効になります。": "This document is signed. Saving after an edit makes the signature invalid.",
  "保護された文書": "Protected Document",
  "この文書はパスワードまたは電子証明書で保護されています。DocuWorks で開いてセキュリティを解除し、保存し直してから、もう一度開いてください。": "This document is protected with a password or a digital certificate. Open it in DocuWorks, remove the security, save it again, and then open it here.",
  "保護された文書です（DocuWorks でセキュリティを解除してから使ってください）": "A protected document (remove the security in DocuWorks first)",
  "開けませんでした: {0}": "Couldn't open: {0}",
  "保存しました（DocuWorks バインダー）": "Saved (DocuWorks binder)",
  "保存しました（DocuWorks 文書）": "Saved (DocuWorks document)",
  "保存できませんでした: {0}": "Couldn't save: {0}",
  "PDF を作っています…": "Making the PDF…",
  "PDF を保存しました": "PDF saved",
  "PDF を作れませんでした: {0}": "Couldn't make the PDF: {0}",
  "{0} / {1}（{2}ページ）": "{0} / {1} (page {2})",
  "見つかりません": "Not found",
  "保存していない変更": "Unsaved Changes",
  "変更が保存されていません。保存しますか？": "Your changes aren't saved. Save them?",
  "画像を貼りました。白い部分は下の文字が透けて見えます。": "Picture added. Text under its white parts shows through.",
  "画像を貼りました。右の「透過」で白い部分を透かせます。": "Picture added. Use See-through on the right to make its white parts clear.",
  "画像を読めませんでした: {0}": "Couldn't read the picture: {0}",
  "{0} ページを挿入しました": ["Inserted {0} page", "Inserted {0} pages"],
  "この種類のファイルは挿入できません": "This kind of file can't be inserted",
  "{0}（{1} ページ）": ["{0} ({1} page)", "{0} ({1} pages)"],
  "PDF を読む部品を取得できませんでした（インターネット接続が必要です）": "Couldn't load the PDF reader (it needs an internet connection)",
  "PDF を読み込んでいます…": "Reading the PDF…",
  "PDF を挿入しています… {0} / {1}": "Inserting PDF pages… {0} / {1}",
  "文書名の変更": "Rename Document",
  "文書名": "Document name",
  "文書を外す": "Remove Document",
  "「{0}」（{1} ページ）をバインダーから外しますか？（元に戻すことができます）": ["Remove \"{0}\" ({1} page) from the binder? (You can undo this.)", "Remove \"{0}\" ({1} pages) from the binder? (You can undo this.)"],
  "{0} 文書を追加しました": ["Added {0} document", "Added {0} documents"],
  "印刷の準備をしています…": "Getting ready to print…",
  "起動できませんでした: {0}": "Couldn't start: {0}",
};

const DICT = LANG === "en" ? EN : null;

/** The words for `ja` in the page's language; {0} {1} … are filled from args.
 * An English entry may be [one, many]: the first number among args picks it. */
function tr(ja, ...args) {
  let s = (DICT && DICT[ja]) || ja;
  if (Array.isArray(s)) s = s[args.find((a) => typeof a === "number") === 1 ? 0 : 1];
  return args.length ? s.replace(/\{(\d+)\}/g, (_, i) => String(args[+i] ?? "")) : s;
}

/** Put the page's static text, titles and labels in the page's language. */
function translateDom(root = document.documentElement) {
  if (DICT) {
    const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode: (n) => (n.parentElement && /^(SCRIPT|STYLE)$/.test(n.parentElement.tagName) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
    });
    for (let n = walk.nextNode(); n; n = walk.nextNode()) {
      const key = n.data.trim();
      if (key && typeof DICT[key] === "string") n.data = n.data.replace(key, DICT[key]);
    }
    for (const el of root.querySelectorAll("[title], [placeholder], [aria-label], [alt]")) {
      for (const a of ["title", "placeholder", "aria-label", "alt"]) {
        const v = el.getAttribute(a);
        if (v && typeof DICT[v.trim()] === "string") el.setAttribute(a, DICT[v.trim()]);
      }
    }
  }
  document.documentElement.classList.add("i18n");
}
