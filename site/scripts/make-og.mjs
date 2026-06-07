// OG sosyal görselini (static/og.svg) PNG'ye çevirir → static/og.png
// Marka/metin değişince yeniden çalıştır: node scripts/make-og.mjs
import { Resvg } from "@resvg/resvg-js";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const svg = readFileSync(join(root, "static", "og.svg"), "utf8");

const resvg = new Resvg(svg, {
  fitTo: { mode: "width", value: 1200 },
  font: { loadSystemFonts: true, defaultFontFamily: "Segoe UI" },
  background: "#000000",
});

writeFileSync(join(root, "static", "og.png"), resvg.render().asPng());
console.log("og.png yazıldı (1200x630)");
