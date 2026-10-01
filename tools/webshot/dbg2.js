const { chromium } = require(require('child_process').execSync('npm root -g').toString().trim() + '/playwright');
(async () => {
  const [editor, file] = process.argv.slice(2);
  const b = await chromium.launch();
  const p = await (await b.newContext({ viewport: { width: 1400, height: 1000 } })).newPage();
  p.on('console', (m) => console.log('console', m.type(), m.text()));
  await p.goto('file://' + editor);
  await p.setInputFiles('#fileinput', file);
  await p.waitForFunction(() => document.querySelectorAll('.page').length > 0);
  await p.waitForTimeout(3000);
  const r = await p.evaluate(async () => {
    const S = window.__ezpzxdw;
    const d = JSON.parse(S.ed.render(0));
    const res = [];
    for (const it of d.items) if (it[0] === 'i') {
      const im = d.images[it[1]];
      const bytes = S.ed.image(0, it[1]);
      let ok = 'n/a';
      try { const bm = im.k === 'jpeg' ? await createImageBitmap(new Blob([bytes], { type: 'image/jpeg' })) : await createImageBitmap(new ImageData(new Uint8ClampedArray(bytes.buffer, bytes.byteOffset, bytes.length), im.w, im.h)); ok = bm.width + 'x' + bm.height; } catch (e) { ok = String(e); }
      res.push([it[1], im.k, im.w, im.h, bytes.length, it[2].map((v) => Math.round(v)).join(','), it[3], ok]);
    }
    return { clips: d.clips.length, res };
  });
  console.log(JSON.stringify(r, null, 0));
  await b.close();
})();
