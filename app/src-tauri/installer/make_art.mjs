// Renders the NSIS installer artwork (sidebar and header, at 2x) from HTML with headless
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

// Drawn at twice the size NSIS lays out (164x314 and 150x57): the installer scales the bitmaps
// to the window, so on high-DPI screens they stay sharp instead of being blown up.
const sidebar = page(328, 628, `
  <div style="position:relative;width:328px;height:628px;overflow:hidden;color:#fff;background:
    radial-gradient(300px 260px at 18% 8%, rgba(77,155,255,.55), transparent 70%),
    radial-gradient(320px 300px at 95% 62%, rgba(46,211,230,.34), transparent 70%),
    radial-gradient(360px 280px at 20% 108%, rgba(165,139,255,.5), transparent 70%),
    linear-gradient(180deg,#070a12,#04060a)">
    <div style="position:absolute;inset:0;background-image:radial-gradient(1.4px 1.4px at 40px 60px,rgba(255,255,255,.5),transparent),radial-gradient(1.2px 1.2px at 210px 190px,rgba(200,225,255,.45),transparent),radial-gradient(1.6px 1.6px at 120px 420px,rgba(255,255,255,.35),transparent),radial-gradient(1.2px 1.2px at 280px 520px,rgba(170,240,255,.4),transparent);background-size:328px 628px"></div>
    <div style="position:absolute;left:44px;top:64px;width:104px;height:104px;border-radius:26px;background:#000;box-shadow:0 0 0 2px rgba(255,255,255,.14),0 18px 48px rgba(0,0,0,.7),0 0 60px rgba(77,155,255,.35);overflow:hidden">
      <div class="logo" style="width:104px;height:104px">${logo}</div>
    </div>
    <div style="position:absolute;left:44px;top:200px;font-size:46px;font-weight:800;letter-spacing:-.02em">Audiora</div>
    <div style="position:absolute;left:46px;top:262px;width:250px;font-size:20px;line-height:1.45;color:rgba(255,255,255,.78)">Pro audio mixer, voice processor and virtual microphone</div>
    <div style="position:absolute;left:46px;top:370px;display:flex;flex-direction:column;gap:14px;font-size:17px;color:rgba(255,255,255,.86)">
      <div><span style="display:inline-block;width:10px;height:10px;border-radius:50%;background:#4d9bff;box-shadow:0 0 10px #4d9bff;margin-right:12px"></span>Channel mixer</div>
      <div><span style="display:inline-block;width:10px;height:10px;border-radius:50%;background:#2ed3e6;box-shadow:0 0 10px #2ed3e6;margin-right:12px"></span>Voice processor</div>
      <div><span style="display:inline-block;width:10px;height:10px;border-radius:50%;background:#a58bff;box-shadow:0 0 10px #a58bff;margin-right:12px"></span>Audiora Microphone</div>
    </div>
    <div style="position:absolute;left:46px;bottom:40px;font-size:15px;letter-spacing:.16em;text-transform:uppercase;color:rgba(255,255,255,.55)">by Roujan<br>Digital Agency</div>
  </div>`);

const header = page(300, 114, `
  <div style="position:relative;width:300px;height:114px;background:#ffffff">
    <div style="position:absolute;right:22px;top:20px;width:74px;height:74px;border-radius:18px;background:#000;overflow:hidden;box-shadow:0 6px 16px rgba(0,0,0,.25)">
      <div class="logo" style="width:74px;height:74px">${logo}</div>
    </div>
    <div style="position:absolute;right:114px;top:28px;font-size:30px;font-weight:800;letter-spacing:-.01em;color:#0b0b0c">Audiora</div>
    <div style="position:absolute;right:114px;top:68px;font-size:14px;letter-spacing:.1em;color:#6b7280">SETUP</div>
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

await render(sidebar, 328, 628, 'sidebar');
await render(header, 300, 114, 'header');
process.exit(0);
