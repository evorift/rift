// OG sosyal görseli (1200x630): yeni kara delik logosu (static/logo.png) gömülü + metin → static/og.png
import { Resvg } from "@resvg/resvg-js";
import { readFileSync, writeFileSync } from "node:fs";

const root = "C:/Users/Evrim/Desktop/projects/net/site";
const logoB64 = readFileSync(root + "/static/logo.png").toString("base64");

const svg = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="1200" height="630" viewBox="0 0 1200 630">
  <defs>
    <radialGradient id="glow" cx="27%" cy="50%" r="44%">
      <stop offset="0%" stop-color="#39e66b" stop-opacity="0.16"/>
      <stop offset="100%" stop-color="#000000" stop-opacity="0"/>
    </radialGradient>
  </defs>
  <rect width="1200" height="630" fill="#000000"/>
  <rect width="1200" height="630" fill="url(#glow)"/>
  <image xlink:href="data:image/png;base64,${logoB64}" href="data:image/png;base64,${logoB64}" x="60" y="115" width="400" height="400" preserveAspectRatio="xMidYMid meet"/>
  <g font-family="'Segoe UI', system-ui, -apple-system, Arial, sans-serif">
    <text x="510" y="298" font-size="120" font-weight="800" fill="#ffffff" letter-spacing="-2">evorift</text>
    <text x="514" y="364" font-size="38" font-weight="600" fill="#9aa39a">VPN olmadan engelleri aş.</text>
    <text x="514" y="420" font-size="27" font-weight="600" fill="#39e66b">Açık kaynak · Ücretsiz · VPN değil</text>
  </g>
</svg>`;

const resvg = new Resvg(svg, {
  fitTo: { mode: "width", value: 1200 },
  font: { loadSystemFonts: true, defaultFontFamily: "Segoe UI" },
  background: "#000000",
});
writeFileSync(root + "/static/og.png", resvg.render().asPng());
console.log("og.png yeni kara delikle yazildi (1200x630)");
