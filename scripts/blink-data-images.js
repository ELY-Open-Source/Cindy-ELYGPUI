// Asks Blink which `data:` addresses an <img> loads, and at what size.
const { app, BrowserWindow, nativeImage } = require('electron');
const fs = require('node:fs');
const path = require('node:path');

const scratch = path.join(__dirname, '..', '.scratch', '9.6i3c', 'electron-data');
app.setPath('userData', scratch);
app.setPath('sessionData', scratch);
app.disableHardwareAcceleration();

const png = nativeImage.createFromBitmap(Buffer.alloc(3 * 2 * 4, 255), { width: 3, height: 2 }).toPNG();
const b64 = png.toString('base64');
const escaped = [...png].map((byte) => `%${byte.toString(16).padStart(2, '0')}`).join('');
const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20"/></svg>';
const svg64 = Buffer.from(svg).toString('base64');
const spaced = b64.replace(/(.{7})/g, '$1 \n\t');
const cases = [
  `data:image/png;base64,${b64}`,
  `data:image/png;BASE64,${b64}`,
  `data:image/png; base64,${b64}`,
  `data:image/png;base64 ,${b64}`,
  `data:image/png;charset=utf-8;base64,${b64}`,
  `data:image/png;base64;charset=utf-8,${b64}`,
  `data:;base64,${b64}`,
  `data:base64,${b64}`,
  `data:,${escaped}`,
  `data:image/png,${escaped}`,
  `data:image/png,${escaped.toUpperCase()}`,
  `data:text/plain;base64,${b64}`,
  `data:image/jpeg;base64,${b64}`,
  `data:application/octet-stream;base64,${b64}`,
  `data:image/png;base64,${spaced}`,
  `data:image/png;base64,${b64.replace(/=+$/, '')}`,
  `data:image/png;base64,${b64}=`,
  `data:image/png;base64,${b64}#frag`,
  `data:image/png;base64,${encodeURIComponent(b64)}`,
  `data:image/png;base64,${b64.replace(/\+/g, '-').replace(/\//g, '_')}`,
  `data:image/png;base64,${b64.slice(0, 20)}!${b64.slice(20)}`,
  `data:image/png;base64,${b64.slice(0, 40)}`,
  `data:image/png;base64,`,
  `data:image/png;base64`,
  `data:image/png`,
  `data:`,
  `DATA:image/png;base64,${b64}`,
  ` data:image/png;base64,${b64} `,
  `data:image/png;base64,${b64}\n`,
  `data:image/png;base64,${b64.slice(0, 30)}\n${b64.slice(30)}`,
  `data:image/svg+xml,${encodeURIComponent(svg)}`,
  `data:image/svg+xml;base64,${svg64}`,
  `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`,
  `data:IMAGE/SVG+XML,${encodeURIComponent(svg)}`,
  `data:image/svg+xml ,${encodeURIComponent(svg)}`,
  `data:image/svg+xml,${svg.replace(/#/g, '%23')}`,
  `data:image/svg+xml,${svg}`,
  `data:image/svg+xml;base64,${b64}`,
  `data:image/png,${encodeURIComponent(svg)}`,
  `data:text/xml,${encodeURIComponent(svg)}`,
  `data:,${encodeURIComponent(svg)}`,
  `data:image/svg,${encodeURIComponent(svg)}`,
  `data:image/svg+xml;base64,${svg64}%`,
  `data:image/png,${escaped}%`,
  `data:image/png,${escaped}%zz`,
];

app.whenReady().then(async () => {
  const win = new BrowserWindow({ show: false, webPreferences: { offscreen: true } });
  await win.loadURL('about:blank');
  const sized = await win.webContents.executeJavaScript(`Promise.all(${JSON.stringify(cases)}.map((src) => new Promise((done) => {
    const img = new Image();
    img.onload = () => done([img.naturalWidth, img.naturalHeight]);
    img.onerror = () => done(null);
    img.src = src;
  })))`);
  const out = cases.map((src, at) => [src, sized[at]]);
  const file = path.join(__dirname, '..', 'tests', 'fixtures', 'data-image', 'blink.json');
  fs.writeFileSync(file, `${JSON.stringify(out, null, 1)}\n`);
  console.log(`${out.length} data image cases, ${sized.filter(Boolean).length} load`);
  app.quit();
});
