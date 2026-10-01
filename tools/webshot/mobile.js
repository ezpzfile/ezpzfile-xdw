const { chromium } = require(require('child_process').execSync('npm root -g').toString().trim() + '/playwright');
(async () => {
  const [editor, file, out] = process.argv.slice(2);
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true });
  const p = await ctx.newPage();
  await p.goto('file://' + editor);
  await p.screenshot({ path: out.replace('.png', '-welcome.png') });
  await p.setInputFiles('#fileinput', file);
  await p.waitForFunction(() => document.querySelectorAll('.page').length > 0);
  await p.waitForTimeout(2500);
  await p.screenshot({ path: out });
  await b.close();
})();
