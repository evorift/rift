// logo.png'yi renkli zemine bindirip şeffaflığı doğrula + köşe/merkez alfa yazdır.
import { Jimp } from "jimp";
import { writeFileSync } from "node:fs";

const LOGO = "C:/Users/Evrim/Desktop/projects/net/src-tauri/icons/logo.png";
const OUT = "C:/Users/Evrim/Desktop/projects/net/site/scripts/_preview.png";

const logo = await Jimp.read(LOGO);
const W = logo.bitmap.width, H = logo.bitmap.height;
const aAt = (x, y) => logo.bitmap.data[(y * W + x) * 4 + 3];
console.log("alfa kose(0,0)=", aAt(0, 0), " kenar(10,H/2)=", aAt(10, H >> 1));

// magenta zemin
const bg = new Jimp({ width: W, height: H, color: 0xff00ffff });
bg.composite(logo, 0, 0);
writeFileSync(OUT, await bg.getBuffer("image/png"));
console.log("preview ->", OUT);
