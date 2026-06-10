// Hafif, bağımsız i18n — uygulamadaki yapının landing sürümü. Harici kütüphane yok.
// `lang` reaktif ($state) → t() çağrıları markup'ta otomatik güncellenir.

export const LANGS = /** @type {const} */ (["tr", "en", "es", "ru"]);
export const LANG_LABEL = { tr: "TR", en: "EN", es: "ES", ru: "RU" };

const tr = {
  "brand.badge": "Açık kaynak · Ücretsiz · Tek tık",
  "nav.features": "Özellikler",
  "nav.how": "Nasıl çalışır",
  "nav.github": "GitHub",
  "hero.title": "Engelleri aş.",
  "hero.sub":
    "Mesajlaşma, sesli konuşma ve oyunları tek tıkla açan Windows aracı. Hızın düşmez — yalnızca engellenen trafiğe dokunur, gerisi doğrudan akar.",
  "hero.hint": "Kara deliğe dokun ↑",
  "cta.download": "Windows için indir",
  "cta.github": "GitHub'da gör",
  "cta.sponsor": "Sponsor ol",
  "cta.soon": "v0.1 çok yakında — şimdilik GitHub'dan takip et",
  "platform": "Windows 10 / 11",
  "feat.title": "Ne yapar?",
  "feat.unblock.t": "Engelleri aşar",
  "feat.unblock.d":
    "DPI-bypass motoru: sesli konuşma, oyunlar ve engellenen uygulamalar yeniden açılır. Sansürü kelimenin tam anlamıyla yarıp geçer.",
  "feat.smart.t": "Akıllı, hızlı yönlendirme",
  "feat.smart.d":
    "Engellenen uygulamalar Cloudflare'in küresel WARP ağından temiz bir yol bulur; gerisi doğrudan kalır. Gereksiz trafik dolaştırılmaz.",
  "feat.safe.t": "Cerrahi ve güvenli",
  "feat.safe.d":
    "Anti-cheat farkında (Vanguard / EAC / BattlEye). Her ayar geri alınabilir; riskli olan önce uyarır.",
  "feat.light.t": "Sıfıra yakın kaynak",
  "feat.light.d":
    "Animasyonlu ama hafif: boştayken %0,3'ten az CPU, ~30 MB RAM. Pencere kapanınca tepside çalışır.",
  "how.title": "Nasıl çalışır?",
  "how.1": "İndir ve çalıştır — kurulum derdi yok, hesap yok.",
  "how.2": "Kara deliğe bas — koruma tek tıkla açılır.",
  "how.3": "Oyna — engellenen uygulamalar açılır, ping'in korunur.",
  "how.tech.title": "Kaputun altında",
  "how.tech.lead":
    "İki teknik birlikte çalışır: çoğu trafik doğrudan akarken engeli aşan paket düzenlemesi, ihtiyaç duyan uygulamalar için Cloudflare WARP üzerinden temiz bir rota.",
  "how.dpi.t": "DPI atlatma",
  "how.dpi.d":
    "Operatörün siteleri bağlantını inceleyerek engeller (derin paket incelemesi / DPI). evorift el sıkışma paketlerini yeniden biçimlendirir; filtre bağlantını sınıflandıramaz, engel hiç tetiklenmez.",
  "how.warp.t": "WARP bölünmüş tünel",
  "how.warp.d":
    "Sesli konuşma gibi bazı uygulamalar temiz bir rota ister. evorift yalnızca o uygulamaları Cloudflare WARP üzerinden gönderir; bağlantının geri kalanı doğrudan kalır.",
  "how.cf.t": "Cloudflare ağı",
  "how.cf.d":
    "WARP, dünyanın en hızlı ağlarından biri olan Cloudflare'in küresel kenarında çalışır — yönlendirilen trafik düşük gecikmede kalır, uzak bir sunucuda yavaşlamaz.",
  "foot.smartscreen":
    "İmzasız sürüm: ilk açılışta Windows SmartScreen uyarısı çıkabilir → 'More info → Run anyway'.",
  "foot.made": "MIT lisansı altında açık kaynak.",
  "foot.tagline": "Engelleri aş.",
  "nav.faq": "SSS",
  "meta.title": "engelleri aş",
  "faq.title": "Sık sorulanlar",
  "faq.q1": "Bağlantımı / ping'imi yavaşlatır mı?",
  "faq.a1":
    "Hayır. Trafiğinin çoğu DPI atlatma ile doğrudan akar. Yalnızca ihtiyaç duyan uygulamalar Cloudflare'in hızlı küresel WARP ağından geçer; böylece hızın ve ping'in korunur.",
  "faq.q2": "Oyunlarda / anti-cheat ile güvenli mi?",
  "faq.a2":
    "Evet. Anti-cheat farkındadır (Vanguard / EAC / BattlEye) ve korumalı bir oyun açılınca otomatik duraklayabilir. Her ayar geri alınabilir.",
  "faq.q3": "Ücretsiz mi?",
  "faq.a3": "Evet, tamamen ücretsiz ve açık kaynak (MIT). Hesap yok, abonelik yok.",
  "faq.q4": "Hangi siteleri/uygulamaları açar?",
  "faq.a4":
    "Mesajlaşma uygulamaları, sesli konuşma, oyunlar ve listene eklediklerin. Hangi alan adlarının etkileneceğini sen seçersin.",
  "faq.q5": "Windows ilk açılışta uyarı veriyor (SmartScreen)?",
  "faq.a5":
    "Ücretsiz kalmak için sürüm imzasız; bu yüzden Windows bir uyarı gösterebilir → 'More info → Run anyway'. İndirme hash'ini doğrulayabilirsin.",
};

const en = {
  "brand.badge": "Open source · Free · One-click",
  "nav.features": "Features",
  "nav.how": "How it works",
  "nav.github": "GitHub",
  "hero.title": "Break through the block.",
  "hero.sub":
    "A Windows tool that unblocks messaging, voice and games in one click. No speed loss — it only touches the traffic that's blocked, everything else stays direct.",
  "hero.hint": "Tap the black hole ↑",
  "cta.download": "Download for Windows",
  "cta.github": "View on GitHub",
  "cta.sponsor": "Sponsor",
  "cta.soon": "v0.1 coming very soon — follow on GitHub for now",
  "platform": "Windows 10 / 11",
  "feat.title": "What it does",
  "feat.unblock.t": "Breaks the block",
  "feat.unblock.d":
    "A DPI-bypass engine: voice calls, games and blocked apps come back to life. It literally rifts through censorship.",
  "feat.smart.t": "Smart, fast routing",
  "feat.smart.d":
    "Blocked apps get a clean path through Cloudflare's global WARP network; everything else stays direct. No detour for traffic that doesn't need it.",
  "feat.safe.t": "Surgical and safe",
  "feat.safe.d":
    "Anti-cheat aware (Vanguard / EAC / BattlEye). Every setting is reversible; risky ones warn you first.",
  "feat.light.t": "Near-zero footprint",
  "feat.light.d":
    "Animated but light: under 0.3% CPU and ~30 MB RAM when idle. Runs in the tray when the window is closed.",
  "how.title": "How it works",
  "how.1": "Download and run — no setup, no account.",
  "how.2": "Hit the black hole — protection turns on in one click.",
  "how.3": "Play — blocked apps open up, your ping stays intact.",
  "how.tech.title": "Under the hood",
  "how.tech.lead":
    "Two techniques work together: packet-level bypass keeps most traffic flowing direct past the block, and a clean route through Cloudflare WARP for the apps that need one.",
  "how.dpi.t": "DPI bypass",
  "how.dpi.d":
    "Your provider blocks sites by inspecting your connection (deep packet inspection). evorift reshapes the handshake packets so the filter can't classify them — the block never triggers, and your traffic stays direct.",
  "how.warp.t": "WARP split-tunnel",
  "how.warp.d":
    "Some apps — like voice calls — need a clean route. evorift sends only those apps through Cloudflare WARP, while the rest of your connection stays direct.",
  "how.cf.t": "Cloudflare network",
  "how.cf.d":
    "WARP runs on Cloudflare's global edge — one of the fastest networks in the world — so routed traffic stays low-latency instead of slowing down on a distant server.",
  "foot.smartscreen":
    "Unsigned build: Windows SmartScreen may warn on first launch → 'More info → Run anyway'.",
  "foot.made": "Open source under the MIT license.",
  "foot.tagline": "Break through the block.",
  "nav.faq": "FAQ",
  "meta.title": "break through the block",
  "faq.title": "Frequently asked",
  "faq.q1": "Will it slow my connection or ping?",
  "faq.a1":
    "No. Most of your traffic stays direct via DPI bypass. Only the apps that need it are routed through Cloudflare's fast global WARP network, so your speed and ping stay intact.",
  "faq.q2": "Is it safe with games / anti-cheat?",
  "faq.a2":
    "Yes. It's anti-cheat aware (Vanguard / EAC / BattlEye) and can auto-pause when a protected game launches. Every setting is reversible.",
  "faq.q3": "Is it free?",
  "faq.a3": "Yes, fully free and open source (MIT). No accounts, no subscriptions.",
  "faq.q4": "Which sites / apps does it unblock?",
  "faq.a4":
    "Messaging apps, voice calls, games, and whatever you add to the list. You choose which domains are affected.",
  "faq.q5": "Windows warns me on first launch (SmartScreen)?",
  "faq.a5":
    "To stay free the build is unsigned, so Windows may show a SmartScreen prompt → 'More info → Run anyway'. You can verify the download hash.",
};

const es = {
  "brand.badge": "Código abierto · Gratis · Un clic",
  "nav.features": "Funciones",
  "nav.how": "Cómo funciona",
  "nav.github": "GitHub",
  "hero.title": "Atraviesa el bloqueo.",
  "hero.sub":
    "Una herramienta de Windows que desbloquea mensajería, voz y juegos con un clic. Sin pérdida de velocidad: solo toca el tráfico bloqueado, el resto va directo.",
  "hero.hint": "Toca el agujero negro ↑",
  "cta.download": "Descargar para Windows",
  "cta.github": "Ver en GitHub",
  "cta.sponsor": "Patrocinar",
  "cta.soon": "v0.1 muy pronto — sigue en GitHub por ahora",
  "platform": "Windows 10 / 11",
  "feat.title": "Qué hace",
  "feat.unblock.t": "Rompe el bloqueo",
  "feat.unblock.d":
    "Un motor anti-DPI: las llamadas de voz, los juegos y las apps bloqueadas vuelven a funcionar. Atraviesa la censura.",
  "feat.smart.t": "Enrutado inteligente y rápido",
  "feat.smart.d":
    "Las apps bloqueadas obtienen una ruta limpia por la red global WARP de Cloudflare; el resto va directo. Sin desvíos para el tráfico que no los necesita.",
  "feat.safe.t": "Quirúrgico y seguro",
  "feat.safe.d":
    "Consciente del anti-cheat (Vanguard / EAC / BattlEye). Cada ajuste es reversible; los riesgosos avisan primero.",
  "feat.light.t": "Huella casi nula",
  "feat.light.d":
    "Animado pero ligero: menos del 0,3 % de CPU y ~30 MB de RAM en reposo. Funciona en la bandeja al cerrar la ventana.",
  "how.title": "Cómo funciona",
  "how.1": "Descarga y ejecuta — sin instalación, sin cuenta.",
  "how.2": "Pulsa el agujero negro — la protección se activa con un clic.",
  "how.3": "Juega — las apps bloqueadas se abren, tu ping se mantiene.",
  "how.tech.title": "Bajo el capó",
  "how.tech.lead":
    "Dos técnicas trabajan juntas: un ajuste a nivel de paquete mantiene la mayoría del tráfico directo saltándose el bloqueo, y una ruta limpia por Cloudflare WARP para las apps que la necesitan.",
  "how.dpi.t": "Evasión de DPI",
  "how.dpi.d":
    "Tu proveedor bloquea sitios inspeccionando tu conexión (inspección profunda de paquetes / DPI). evorift reescribe los paquetes del saludo para que el filtro no pueda clasificarlos — el bloqueo nunca se activa y tu tráfico va directo.",
  "how.warp.t": "Túnel dividido WARP",
  "how.warp.d":
    "Algunas apps —como las llamadas de voz— necesitan una ruta limpia. evorift envía solo esas apps por Cloudflare WARP, mientras el resto de tu conexión va directo.",
  "how.cf.t": "Red de Cloudflare",
  "how.cf.d":
    "WARP corre en el edge global de Cloudflare —una de las redes más rápidas del mundo—, así el tráfico enrutado mantiene baja latencia en vez de frenarse en un servidor lejano.",
  "foot.smartscreen":
    "Build sin firmar: Windows SmartScreen puede avisar al inicio → 'Más información → Ejecutar de todos modos'.",
  "foot.made": "Código abierto bajo licencia MIT.",
  "foot.tagline": "Atraviesa el bloqueo.",
  "nav.faq": "FAQ",
  "meta.title": "atraviesa el bloqueo",
  "faq.title": "Preguntas frecuentes",
  "faq.q1": "¿Ralentiza mi conexión o mi ping?",
  "faq.a1":
    "No. La mayor parte de tu tráfico va directo gracias a la evasión de DPI. Solo las apps que lo necesitan se enrutan por la rápida red global WARP de Cloudflare, así tu velocidad y tu ping se mantienen.",
  "faq.q2": "¿Es seguro con juegos / anti-cheat?",
  "faq.a2":
    "Sí. Es consciente del anti-cheat (Vanguard / EAC / BattlEye) y puede pausarse solo cuando se abre un juego protegido. Cada ajuste es reversible.",
  "faq.q3": "¿Es gratis?",
  "faq.a3": "Sí, totalmente gratis y de código abierto (MIT). Sin cuentas, sin suscripciones.",
  "faq.q4": "¿Qué sitios / apps desbloquea?",
  "faq.a4":
    "Apps de mensajería, llamadas de voz, juegos y lo que añadas a la lista. Tú eliges qué dominios se ven afectados.",
  "faq.q5": "¿Windows me avisa al iniciar (SmartScreen)?",
  "faq.a5":
    "Para seguir siendo gratis la build no está firmada, así que Windows puede mostrar un aviso de SmartScreen → 'Más información → Ejecutar de todos modos'. Puedes verificar el hash de la descarga.",
};

const ru = {
  "brand.badge": "Открытый код · Бесплатно · Один клик",
  "nav.features": "Возможности",
  "nav.how": "Как это работает",
  "nav.github": "GitHub",
  "hero.title": "Пробей блокировку.",
  "hero.sub":
    "Программа для Windows, которая разблокирует мессенджеры, голосовые вызовы и игры одним кликом. Без потери скорости — затрагивает только заблокированный трафик, остальное идёт напрямую.",
  "hero.hint": "Коснитесь чёрной дыры ↑",
  "cta.download": "Скачать для Windows",
  "cta.github": "Открыть на GitHub",
  "cta.sponsor": "Спонсировать",
  "cta.soon": "v0.1 уже совсем скоро — пока следите на GitHub",
  "platform": "Windows 10 / 11",
  "feat.title": "Что она делает",
  "feat.unblock.t": "Снимает блокировку",
  "feat.unblock.d":
    "Движок обхода DPI: голосовые вызовы, игры и заблокированные приложения снова работают. Буквально прорывает цензуру.",
  "feat.smart.t": "Умная, быстрая маршрутизация",
  "feat.smart.d":
    "Заблокированные приложения получают чистый путь через глобальную сеть Cloudflare WARP; остальное идёт напрямую. Лишний трафик никуда не заворачивается.",
  "feat.safe.t": "Точно и безопасно",
  "feat.safe.d":
    "Учитывает анти-чит (Vanguard / EAC / BattlEye). Любую настройку можно откатить; рискованные предупреждают.",
  "feat.light.t": "Почти нулевая нагрузка",
  "feat.light.d":
    "Анимировано, но легко: меньше 0,3 % CPU и ~30 МБ ОЗУ в простое. При закрытии окна работает в трее.",
  "how.title": "Как это работает",
  "how.1": "Скачайте и запустите — без установки и аккаунта.",
  "how.2": "Нажмите на чёрную дыру — защита включается одним кликом.",
  "how.3": "Играйте — заблокированные приложения открываются, пинг сохраняется.",
  "how.tech.title": "Под капотом",
  "how.tech.lead":
    "Две техники работают вместе: правка пакетов на лету пропускает большую часть трафика напрямую мимо блокировки, а для приложений, которым нужно, — чистый маршрут через Cloudflare WARP.",
  "how.dpi.t": "Обход DPI",
  "how.dpi.d":
    "Провайдер блокирует сайты, разбирая ваше соединение (глубокая инспекция пакетов / DPI). evorift переформирует пакеты рукопожатия, и фильтр не может их классифицировать — блокировка не срабатывает, а трафик идёт напрямую.",
  "how.warp.t": "Раздельный туннель WARP",
  "how.warp.d":
    "Некоторым приложениям — например, голосовым вызовам — нужен чистый маршрут. evorift отправляет через Cloudflare WARP только эти приложения, а остальное соединение идёт напрямую.",
  "how.cf.t": "Сеть Cloudflare",
  "how.cf.d":
    "WARP работает на глобальном edge Cloudflare — одной из самых быстрых сетей в мире — поэтому маршрутизированный трафик остаётся с низкой задержкой, а не тормозит на далёком сервере.",
  "foot.smartscreen":
    "Сборка без подписи: при первом запуске Windows SmartScreen может предупредить → 'Подробнее → Выполнить в любом случае'.",
  "foot.made": "Открытый код под лицензией MIT.",
  "foot.tagline": "Пробей блокировку.",
  "nav.faq": "ЧаВо",
  "meta.title": "пробей блокировку",
  "faq.title": "Частые вопросы",
  "faq.q1": "Замедлит ли это соединение или пинг?",
  "faq.a1":
    "Нет. Большая часть трафика идёт напрямую благодаря обходу DPI. Только нужные приложения маршрутизируются через быструю глобальную сеть Cloudflare WARP, поэтому скорость и пинг сохраняются.",
  "faq.q2": "Безопасно с играми / анти-читом?",
  "faq.a2":
    "Да. Учитывает анти-чит (Vanguard / EAC / BattlEye) и может сам приостановиться при запуске защищённой игры. Любую настройку можно откатить.",
  "faq.q3": "Это бесплатно?",
  "faq.a3": "Да, полностью бесплатно и с открытым кодом (MIT). Без аккаунтов и подписок.",
  "faq.q4": "Какие сайты / приложения разблокирует?",
  "faq.a4":
    "Мессенджеры, голосовые вызовы, игры и всё, что вы добавите в список. Вы сами выбираете, какие домены затрагиваются.",
  "faq.q5": "Windows предупреждает при первом запуске (SmartScreen)?",
  "faq.a5":
    "Чтобы оставаться бесплатной, сборка без подписи, поэтому Windows может показать предупреждение → 'Подробнее → Выполнить в любом случае'. Можно проверить хеш загрузки.",
};

const dict = { tr, en, es, ru };

let lang = $state("tr");

export function getLang() {
  return lang;
}

export function setLang(l) {
  if (!LANGS.includes(l)) return;
  lang = l;
  if (typeof document !== "undefined") document.documentElement.lang = l;
  try {
    localStorage.setItem("rift-lang", l);
  } catch {}
}

export function initLang() {
  try {
    const saved = localStorage.getItem("rift-lang");
    if (saved && LANGS.includes(saved)) return setLang(saved);
  } catch {}
  const nav = (typeof navigator !== "undefined" && navigator.language || "tr").slice(0, 2);
  setLang(LANGS.includes(nav) ? nav : "tr");
}

export function t(key) {
  return dict[lang][key] ?? dict.tr[key] ?? key;
}
