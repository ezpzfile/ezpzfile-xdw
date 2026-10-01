const { chromium } = require(require('child_process').execSync('npm root -g').toString().trim() + '/playwright');
(async () => {
  const [editor, file, out] = process.argv.slice(2);
  const b = await chromium.launch();
  const p = await (await b.newContext({ viewport: { width: 1400, height: 1400 } })).newPage();
  p.on('console', (m) => console.log('console', m.type(), m.text()));
  p.on('pageerror', (e) => console.log('pageerror', String(e)));
  await p.goto('file://' + editor);
  await p.setInputFiles('#fileinput', file);
  await p.waitForFunction(() => document.querySelectorAll('.page').length > 0);
  for (let t = 0; t < 20; t++) {
    await p.waitForTimeout(1000);
    const st = await p.evaluate(() => window.__ezxdw.pages.map((x) => x.drawn));
    if (st[0]) { console.log('drawn after', t + 1, 's'); break; }
  }
  const c = await p.$$('.page'); await c[0].screenshot({ path: out });
  await b.close();
})();
