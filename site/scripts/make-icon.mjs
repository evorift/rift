// Uygulama ikonu kaynağını üretir: src-tauri/icons/logo.svg → logo.png (1024x1024)
// Sonra: npm run tauri icon src-tauri/icons/logo.png  (tüm boyutları + .ico üretir)
import { Resvg } from "@resvg/resvg-js";
import { readFileSync, writeFileSync } from "node:fs";

const SRC = "C:/Users/Evrim/Desktop/projects/net/src-tauri/icons/logo.svg";
const OUT = "C:/Users/Evrim/Desktop/projects/net/src-tauri/icons/logo.png";

const svg = readFileSync(SRC, "utf8");
const resvg = new Resvg(svg, { fitTo: { mode: "width", value: 1024 } });
writeFileSync(OUT, resvg.render().asPng());
console.log("logo.png yazıldı (1024x1024)");
