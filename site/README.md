# evorift — landing site

evorift'in tanıtım sayfası. SvelteKit + `adapter-static` ile **tamamen statik** build (sunucu yok). Hero'daki kara delik, uygulamadaki `BlackHole.svelte`'in dekoratif kopyasıdır (Three.js/WebGL) — orijinal dosyaya dokunulmadı.

## Geliştirme

```bash
cd site
npm install
npm run dev      # http://localhost:5173
```

## Build

```bash
npm run build    # çıktı: site/build/
npm run preview  # statik çıktıyı önizle
```

## Yayınlama (bedava)

Çıktı (`build/`) host-bağımsızdır; kökten servis edilir.

### Cloudflare Pages (önerilen — temiz URL: `evorift.pages.dev`)
1. Cloudflare hesabı → **Pages → Connect to Git** → `evorift/rift` reposu.
2. Build ayarları:
   - **Root directory:** `site`
   - **Build command:** `npm run build`
   - **Output directory:** `build`
3. Deploy. URL: `https://<seçtiğin-ad>.pages.dev`. İstersen özel alan adı eklenir (alan adının kendisi ~$10/yıl).

### GitHub Pages (alt-klasör `/rift` — yedek)
`.github/workflows/site.yml` hazır. Repo **Settings → Pages → Source: GitHub Actions**.
Alt-klasörde servis edildiği için base path ile build edilir:

```bash
BASE_PATH=/rift npm run build
```

URL: `https://evorift.github.io/rift/`.

> Not: Cloudflare Pages kök dizinden servis ettiği için `BASE_PATH` **gerekmez** (varsayılan boş).

## Diller
TR / EN / ES / RU — `src/lib/i18n.svelte.js`. Sağ üstten anında değişir, tercih `localStorage`'da tutulur.
