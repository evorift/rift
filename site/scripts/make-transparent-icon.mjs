// Siyah zeminli beyaz kara-delik → DIŞ arka plan ŞEFFAF (iç siyah disk KALIR),
// sonra markayı KIRP + büyüt: en büyük bağlı bileşeni (gezegen+halka) bulup
// kareye ~%90 dolduracak şekilde ortalar. Dağınık uzak noktalar kırpılır.
import { Jimp } from "jimp";
import { writeFileSync } from "node:fs";

const SRC = "C:/Users/Evrim/Downloads/Gemini_Generated_Image_z62jmrz62jmrz62j.jpg";
const OUT = "C:/Users/Evrim/Desktop/projects/net/src-tauri/icons/logo.png";

const T = 100;       // bu parlaklığın altı "karanlık" (flood-fill geçer)
const BLACK_T = 70;  // şekil içinde bu altı saf siyaha çekilir (delik temiz)
const FILL = 0.92;   // marka karenin bu oranını kaplasın
const OUTSIZE = 1024;

const img = await Jimp.read(SRC);
const W = img.bitmap.width, H = img.bitmap.height;
const d = img.bitmap.data;
const lum = (i) => Math.max(d[i * 4], d[i * 4 + 1], d[i * 4 + 2]);

// 1) kenarlardan flood-fill → border'a bağlı karanlık = arka plan
const bg = new Uint8Array(W * H);
let st = [];
const seed = (i) => { if (!bg[i] && lum(i) < T) { bg[i] = 1; st.push(i); } };
for (let x = 0; x < W; x++) { seed(x); seed((H - 1) * W + x); }
for (let y = 0; y < H; y++) { seed(y * W); seed(y * W + (W - 1)); }
while (st.length) {
  const i = st.pop(), x = i % W, y = (i - x) / W;
  if (x > 0) seed(i - 1);
  if (x < W - 1) seed(i + 1);
  if (y > 0) seed(i - W);
  if (y < H - 1) seed(i + W);
}

// 2) alfa + delik temizliği
for (let i = 0; i < W * H; i++) {
  const o = i * 4;
  if (bg[i]) d[o + 3] = 0;
  else { d[o + 3] = 255; if (lum(i) < BLACK_T) { d[o] = d[o + 1] = d[o + 2] = 0; } }
}

// 3) en büyük opak bağlı bileşen (ana marka) → bbox
const seen = new Uint8Array(W * H);
let best = { area: 0, x0: 0, x1: 0, y0: 0, y1: 0 };
const stack = [];
for (let s = 0; s < W * H; s++) {
  if (bg[s] || seen[s]) continue;
  let area = 0, x0 = W, x1 = 0, y0 = H, y1 = 0;
  stack.length = 0; stack.push(s); seen[s] = 1;
  while (stack.length) {
    const i = stack.pop(), x = i % W, y = (i - x) / W;
    area++;
    if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y;
    if (x > 0 && !bg[i - 1] && !seen[i - 1]) { seen[i - 1] = 1; stack.push(i - 1); }
    if (x < W - 1 && !bg[i + 1] && !seen[i + 1]) { seen[i + 1] = 1; stack.push(i + 1); }
    if (y > 0 && !bg[i - W] && !seen[i - W]) { seen[i - W] = 1; stack.push(i - W); }
    if (y < H - 1 && !bg[i + W] && !seen[i + W]) { seen[i + W] = 1; stack.push(i + W); }
  }
  if (area > best.area) best = { area, x0, x1, y0, y1 };
}

// 4) bbox merkezine kare kırp (FILL oranıyla), şeffaf tuvale ortala, OUTSIZE'a indir
const bw = best.x1 - best.x0 + 1, bh = best.y1 - best.y0 + 1;
const side = Math.round(Math.max(bw, bh) / FILL);
const cx = (best.x0 + best.x1) / 2, cy = (best.y0 + best.y1) / 2;
const ox = Math.round(cx - side / 2), oy = Math.round(cy - side / 2);

const out = new Jimp({ width: side, height: side, color: 0x00000000 });
out.composite(img, -ox, -oy);
out.resize({ w: OUTSIZE, h: OUTSIZE });
writeFileSync(OUT, await out.getBuffer("image/png"));
console.log(`marka bbox ${bw}x${bh} -> kare ${side} -> ${OUTSIZE}px (dolum ~${Math.round(FILL * 100)}%)`);
