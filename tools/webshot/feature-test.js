// Drive the editor through the newer features: sticky note, date stamp,
// pages inserted from a blank page / a picture / another .xdw / a PDF, and
// (for a .xbd) binder documents. Screenshots and saved files go to OUTDIR.
// usage: node feature-test.js EDITOR.html FILE.xdw OUTDIR PICTURE.jpg OTHER.xdw [DOC.pdf]
//        node feature-test.js EDITOR.html FILE.xbd OUTDIR - OTHER.xdw
const { chromium } = require(require('child_process').execSync('npm root -g').toString().trim() + '/playwright');
const path = require('path');
(async () => {
  const [editor, file, out, picture, other, pdf] = process.argv.slice(2);
  const proxy = process.env.HTTPS_PROXY ? { server: process.env.HTTPS_PROXY } : undefined;
  const browser = await chromium.launch(proxy ? { proxy } : {});
  const ctx = await browser.newContext({ viewport: { width: 1400, height: 1000 }, acceptDownloads: true, ignoreHTTPSErrors: true });
  const p = await ctx.newPage();
  const errors = [];
  p.on('pageerror', (e) => errors.push(String(e)));
  p.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await p.goto('file://' + editor);
  await p.setInputFiles('#fileinput', file);
  await p.waitForFunction(() => document.querySelectorAll('.page').length > 0);
  await p.waitForTimeout(1500);
  const binder = file.endsWith('.xbd');
  const pageBox = async (k) => (await p.$$('.page'))[k].boundingBox();
  const menu = async (name, item) => {
    await p.click(`#menubar .menu > button:has-text("${name}")`);
    await p.click(`.menu.open .drop button:has-text("${item}")`);
  };
  const count = () => p.evaluate(() => window.__ezxdw.info.length);
  const log = (...a) => console.log(...a);

  if (!binder) {
    let box = await pageBox(0);
    const at = (fx, fy) => [box.x + box.width * fx, box.y + box.height * fy];
    // sticky note
    await p.keyboard.press('s');
    let [x, y] = at(0.58, 0.05);
    await p.mouse.click(x, y);
    await p.waitForSelector('.textedit');
    await p.keyboard.type('確認お願いします');
    await p.keyboard.press('Enter');
    await p.keyboard.type('10/3 まで');
    await p.keyboard.press('Control+Enter');
    await p.waitForTimeout(800);
    // date stamp
    await p.keyboard.press('d');
    [x, y] = at(0.86, 0.09);
    await p.mouse.click(x, y);
    await p.waitForSelector('#dlg-stamp[open]');
    await p.fill('#stamp-top', '受付');
    await p.fill('#stamp-bottom', '山田');
    await p.click('#dlg-stamp button[value=ok]');
    await p.waitForTimeout(1000);
    await p.keyboard.press('Escape');
    await p.screenshot({ path: path.join(out, 'f-1.png') });
    log('annotations', await p.evaluate(() => window.__ezxdw.info[0].objects.map((o) => o.kind + (o.shape ? ':' + o.shape.type : '')).join(', ')));

    // blank page after page 1
    const n0 = await count();
    await menu('ページ', '白紙ページを挿入');
    await p.waitForTimeout(800);
    log('blank page', n0, '→', await count());
    // picture as a page, after the current page
    let [fc] = await Promise.all([p.waitForEvent('filechooser'), menu('ページ', 'ファイルからページを挿入')]);
    await fc.setFiles(picture);
    await p.waitForFunction((n) => window.__ezxdw.info.length > n, n0 + 1, { timeout: 30000 });
    log('picture page →', await count());
    // pages of another .xdw (all)
    const n1 = await count();
    [fc] = await Promise.all([p.waitForEvent('filechooser'), menu('ページ', 'ファイルからページを挿入')]);
    await fc.setFiles(other);
    await p.waitForTimeout(800);
    if (await p.$('#dlg-range[open]')) await p.click('#dlg-range button[value=ok]');
    await p.waitForFunction((n) => window.__ezxdw.info.length > n, n1, { timeout: 30000 });
    log('other .xdw →', await count());
    // a PDF (needs pdf.js from the network)
    if (pdf) {
      const n2 = await count();
      [fc] = await Promise.all([p.waitForEvent('filechooser'), menu('ページ', 'ファイルからページを挿入')]);
      await fc.setFiles(pdf);
      try {
        await p.waitForSelector('#dlg-range[open]', { timeout: 30000 });
        await p.check('#dlg-range input[value=some]');
        await p.fill('#range-text', '1');
        await p.click('#dlg-range button[value=ok]');
      } catch (e) { log('no range dialog', String(e).slice(0, 80)); }
      try {
        await p.waitForFunction((n) => window.__ezxdw.info.length > n, n2, { timeout: 90000 });
      } catch (e) { log('PDF not inserted'); }
      log('pdf →', await count());
    }
    await p.waitForTimeout(1500);
    // look at the inserted pages
    for (let k = 1; k < Math.min(await count(), 5); k++) {
      await p.evaluate((k) => document.querySelectorAll('.page')[k].scrollIntoView(), k);
      await p.waitForTimeout(1200);
      await p.screenshot({ path: path.join(out, `f-p${k + 1}.png`) });
    }
  } else {
    log('binder', await p.evaluate(() => JSON.stringify(window.__ezxdw.binder)));
    // add a document
    let [fc] = await Promise.all([p.waitForEvent('filechooser'), p.click('#binder-add')]);
    await fc.setFiles(other);
    await p.waitForTimeout(1200);
    // rename the first document
    await p.dblclick('#pagelist li.doc >> nth=0');
    await p.waitForSelector('#dlg-prompt[open]');
    await p.fill('#pr-input', '表紙と案内');
    await p.click('#dlg-prompt button[value=ok]');
    await p.waitForTimeout(800);
    // move the last document up
    const n = await p.evaluate(() => window.__ezxdw.binder.docs.length);
    await p.hover(`#pagelist li.doc >> nth=${n - 1}`);
    await p.click(`#pagelist li.doc >> nth=${n - 1} >> button[title="文書を上へ"]`);
    await p.waitForTimeout(1200);
    log('binder after', await p.evaluate(() => JSON.stringify(window.__ezxdw.binder.docs.map((d) => [d.name, d.first_page, d.pages]))));
    await p.screenshot({ path: path.join(out, 'b-1.png') });
  }

  const [dl] = await Promise.all([p.waitForEvent('download'), p.keyboard.press('Control+s')]);
  await dl.saveAs(path.join(out, binder ? 'saved.xbd' : 'saved.xdw'));
  log('saved', dl.suggestedFilename());
  if (errors.length) log('ERRORS', errors.join('\n'));
  await browser.close();
})();
