// Blink's fast line break table: run with Cindy's Electron, writes resources/line-break-pairs.txt.
const { app, BrowserWindow } = require('electron');
const fs = require('node:fs');
const path = require('node:path');

const [MIN, MAX] = [0x21, 0xff];
const probe = (min, max) => {
  const host = document.createElement('div');
  host.style.cssText = 'white-space:pre-wrap;font:15px/20px monospace;width:1px';
  for (let a = min; a <= max; a++) {
    for (let b = min; b <= max; b++) {
      const cell = document.createElement('div');
      cell.textContent = String.fromCharCode(a, b);
      host.appendChild(cell);
    }
  }
  document.body.appendChild(host);
  const rows = [];
  let cell = host.firstChild;
  for (let a = min; a <= max; a++) {
    let row = '';
    for (let b = min; b <= max; b++) {
      row += cell.offsetHeight > 20 ? '1' : '0';
      cell = cell.nextSibling;
    }
    rows.push(row);
  }
  return rows;
};

app.disableHardwareAcceleration();
app.whenReady().then(async () => {
  const win = new BrowserWindow({ show: false, width: 1200, height: 800 });
  await win.loadURL('about:blank');
  const rows = await win.webContents.executeJavaScript(`(${probe.toString()})(${MIN}, ${MAX})`);
  const out = path.join(__dirname, '..', 'resources', 'line-break-pairs.txt');
  fs.writeFileSync(out, `${rows.join('\n')}\n`);
  console.log(`chrome ${process.versions.chrome}: ${rows.length} rows`);
  app.quit();
});
