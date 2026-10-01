// Drive the editor like a person: add annotations, move one, rotate a page,
// then save as .xdw and PDF. Screenshots and saved files go to OUTDIR.
// usage: node edit-test.js EDITOR.html FILE.xdw OUTDIR
const { chromium } = require(require('child_process').execSync('npm root -g').toString().trim() + '/playwright');
const path = require('path');
(async () => {
  const [editor, file, out] = process.argv.slice(2);
  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 1400, height: 1000 }, acceptDownloads: true });
  const p = await ctx.newPage();
  const errors = [];
  p.on('pageerror', (e) => errors.push(String(e)));
  p.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await p.goto('file://' + editor);
  await p.setInputFiles('#fileinput', file);
  await p.waitForFunction(() => document.querySelectorAll('.page').length > 0);
  await p.waitForTimeout(1500);
  const page1 = (await p.$$('.page'))[0];
  const box = await page1.boundingBox();
  const at = (fx, fy) => [box.x + box.width * fx, box.y + box.height * fy];

  // 1. text annotation
  await p.keyboard.press('t');
  let [x, y] = at(0.55, 0.06);
  await p.mouse.click(x, y);
  await p.waitForSelector('.textedit');
  await p.keyboard.type('EZPZ 注釈テスト');
  await p.keyboard.press('Control+Enter');
  await p.waitForTimeout(800);

  // 2. highlighter over the title
  await p.keyboard.press('h');
  [x, y] = at(0.40, 0.115);
  await p.mouse.move(x, y); await p.mouse.down();
  [x, y] = at(0.78, 0.14);
  await p.mouse.move(x, y, { steps: 5 }); await p.mouse.up();
  await p.waitForTimeout(800);

  // 3. rectangle and 4. line
  await p.keyboard.press('r');
  [x, y] = at(0.6, 0.30); await p.mouse.move(x, y); await p.mouse.down();
  [x, y] = at(0.9, 0.38); await p.mouse.move(x, y, { steps: 5 }); await p.mouse.up();
  await p.waitForTimeout(600);
  await p.keyboard.press('l');
  [x, y] = at(0.1, 0.45); await p.mouse.move(x, y); await p.mouse.down();
  [x, y] = at(0.5, 0.50); await p.mouse.move(x, y, { steps: 5 }); await p.mouse.up();
  await p.waitForTimeout(600);
  await p.keyboard.press('e');
  [x, y] = at(0.62, 0.42); await p.mouse.move(x, y); await p.mouse.down();
  [x, y] = at(0.88, 0.50); await p.mouse.move(x, y, { steps: 5 }); await p.mouse.up();
  await p.waitForTimeout(600);

  // 5. select the rectangle and move it
  await p.keyboard.press('Escape');
  [x, y] = at(0.6, 0.30);
  await p.mouse.move(x, y + 2); await p.mouse.down();
  await p.mouse.move(x + 20, y + 40, { steps: 5 }); await p.mouse.up();
  await p.waitForTimeout(800);
  await p.screenshot({ path: path.join(out, 'edit-1.png') });

  // 6. rotate page 2 right
  await p.evaluate(() => { document.querySelectorAll('#pagelist li')[1].scrollIntoView(); });
  await p.hover('#pagelist li:nth-child(2)');
  await p.click('#pagelist li:nth-child(2) .tools button:nth-child(2)');
  await p.waitForTimeout(1500);
  await p.evaluate(() => { document.querySelectorAll('.page')[1].scrollIntoView(); });
  await p.waitForTimeout(1500);
  await p.screenshot({ path: path.join(out, 'edit-2.png') });

  // 7. save .xdw
  let [dl] = await Promise.all([p.waitForEvent('download'), p.keyboard.press('Control+s')]);
  await dl.saveAs(path.join(out, 'saved.xdw'));
  // 8. PDF
  await p.click('#menubar .menu:nth-child(1) > button');
  [dl] = await Promise.all([p.waitForEvent('download', { timeout: 120000 }), p.click('text=PDF として保存')]);
  await dl.saveAs(path.join(out, 'saved.pdf'));
  const state = await p.evaluate(() => ({ title: document.title, pages: document.querySelectorAll('.page').length }));
  console.log(JSON.stringify(state));
  if (errors.length) console.log('ERRORS', errors.join('\n'));
  await browser.close();
})();
