// Open a .xdw in the built editor (headless Chromium) and take screenshots.
// usage: node shot.js EDITOR.html FILE.xdw OUT.png [page] [--edit]
const { chromium } = require(require('child_process').execSync('npm root -g').toString().trim() + '/playwright');
(async () => {
  const [editor, file, out, page = '1'] = process.argv.slice(2);
  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 1400, height: 1000 }, deviceScaleFactor: 1 });
  const p = await ctx.newPage();
  const errors = [];
  p.on('pageerror', (e) => errors.push(String(e)));
  p.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await p.goto('file://' + editor);
  await p.waitForSelector('#welcome-open');
  await p.setInputFiles('#fileinput', file);
  await p.waitForFunction(() => document.querySelectorAll('.page').length > 0, null, { timeout: 60000 });
  const n = +page - 1;
  await p.evaluate((n) => { const pg = document.querySelectorAll('.page')[n]; pg && pg.scrollIntoView(); }, n);
  await p.waitForTimeout(2500);
  await p.screenshot({ path: out });
  // also the page canvas alone at higher zoom
  const c = await p.$$('.page');
  if (c[n]) await c[n].screenshot({ path: out.replace(/\.png$/, '-page.png') });
  if (errors.length) console.log('ERRORS', errors.join('\n'));
  await browser.close();
})();
