// Renders the NSIS installer artwork (sidebar 164x314, header 150x57) from HTML with headless
// Edge, then converts the PNGs to the 24-bit BMPs NSIS needs. Re-run after a logo change:
//   node app/src-tauri/installer/make_art.mjs
import { spawn, execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const logo = readFileSync(join(here, '..', 'icon-source.svg'), 'utf8').replace(/width="1024" height="1024"/, 'width="100%" height="100%"');
const edge = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';

const page = (w, h, body) => `<!doctype html><html><head><meta charset="utf-8"><style>
  html,body{margin:0;width:${w}px;height:${h}px;overflow:hidden;font-family:'Segoe UI Variable','Segoe UI',sans-serif}
  .logo{display:block}
</style></head><body>${body}</body></html>`;

const sidebar = page(164, 314, `
  <div style="position:relative;width:164px;height:314px;background:
    radial-gradient(160px 140px at 20% 10%, rgba(77,155,255,.45), transparent 70%),
    radial-gradient(160px 160px at 90% 70%, rgba(46,211,230,.32), transparent 70%),
    radial-gradient(180px 140px at 30% 110%, rgba(165,139,255,.40), transparent 70%),
    #05070b;color:#fff">
    <div class="logo" style="position:absolute;left:22px;top:30px;width:52px;height:52px;border-radius:12px;box-shadow:0 8px 24px rgba(0,0,0,.6)">${logo}</div>
    <div style="position:absolute;left:22px;top:100px;font-size:24px;font-weight:700;letter-spacing:-.01em">Audiora</div>
    <div style="position:absolute;left:22px;top:132px;width:124px;font-size:11px;line-height:1.45;color:rgba(255,255,255,.72)">Mixer, voice studio and recorder for Windows</div>
    <div style="position:absolute;left:22px;bottom:22px;font-size:9.5px;letter-spacing:.14em;text-transform:uppercase;color:rgba(255,255,255,.55)">Roujan Digital<br>Agency</div>
  </div>`);

const header = page(150, 57, `
  <div style="position:relative;width:150px;height:57px;background:#ffffff">
    <div class="logo" style="position:absolute;right:12px;top:10px;width:37px;height:37px">${logo}</div>
    <div style="position:absolute;right:58px;top:12px;font-size:15px;font-weight:700;color:#0b0b0c">Audiora</div>
    <div style="position:absolute;right:58px;top:32px;font-size:8px;letter-spacing:.08em;color:#6b7280">ROUJAN DIGITAL</div>
  </div>`);

const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const work = mkdtempSync(join(tmpdir(), 'audiora-art-'));

async function render(html, w, h, out) {
  const file = join(work, `${out}.html`);
  writeFileSync(file, html);
  const proc = spawn(edge, ['--headless=new', '--remote-debugging-port=9335', `--user-data-dir=${join(work, 'profile')}`, '--hide-scrollbars', 'about:blank'], { stdio: 'ignore' });
  let list;
  for (let i = 0; i < 40; i++) {
    try {
      list = await (await fetch('http://127.0.0.1:9335/json')).json();
      if (list.some((t) => t.type === 'page')) break;
    } catch {}
    await wait(250);
  }
  const ws = new WebSocket(list.find((t) => t.type === 'page').webSocketDebuggerUrl);
  await new Promise((r) => (ws.onopen = r));
  let id = 0;
  const pending = new Map();
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && pending.has(d.id)) pending.get(d.id)(d);
  };
  const send = (method, params = {}) => new Promise((res) => { const i = ++id; pending.set(i, res); ws.send(JSON.stringify({ id: i, method, params })); });
  await send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
  await send('Page.navigate', { url: 'file:///' + file.replace(/\\/g, '/') });
  await wait(1200);
  const s = await send('Page.captureScreenshot', { format: 'png', clip: { x: 0, y: 0, width: w, height: h, scale: 1 } });
  const png = join(work, `${out}.png`);
  writeFileSync(png, Buffer.from(s.result.data, 'base64'));
  ws.close();
  proc.kill();
  await wait(500);
  // NSIS wants 24-bit BMPs
  const bmp = join(here, `${out}.bmp`);
  execFileSync('powershell', ['-NoProfile', '-Command',
    `Add-Type -AssemblyName System.Drawing; $i=[Drawing.Image]::FromFile('${png}'); $b=New-Object Drawing.Bitmap($i.Width,$i.Height,[Drawing.Imaging.PixelFormat]::Format24bppRgb); $g=[Drawing.Graphics]::FromImage($b); $g.DrawImage($i,0,0,$i.Width,$i.Height); $b.Save('${bmp}',[Drawing.Imaging.ImageFormat]::Bmp); $g.Dispose(); $b.Dispose(); $i.Dispose()`]);
  console.log('wrote', bmp);
}

await render(sidebar, 164, 314, 'sidebar');
await render(header, 150, 57, 'header');
process.exit(0);
