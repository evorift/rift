import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

// Host-bağımsız: kökten servis edilir (Cloudflare Pages, özel alan adı, Netlify).
// GitHub Pages alt-klasör (örn. /rift) için: BASE_PATH=/rift npm run build
const base = process.env.BASE_PATH ?? "";

/** @type {import('@sveltejs/kit').Config} */
export default {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({ fallback: "404.html" }),
    paths: { base },
  },
};
