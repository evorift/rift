// Hafif, bağımsız i18n — uygulamadaki yapının landing sürümü. Harici kütüphane yok.
// `lang` reaktif ($state) → t() çağrıları markup'ta otomatik güncellenir.

export const LANGS = /** @type {const} */ (["tr", "en", "es", "ru"]);
export const LANG_LABEL = { tr: "TR", en: "EN", es: "ES", ru: "RU" };

const tr = {
  "brand.badge": "Açık kaynak · Ücretsiz · VPN değil",
  "nav.features": "Özellikler",
  "nav.how": "Nasıl çalışır",
  "nav.github": "GitHub",
  "hero.title": "Engelleri aş. VPN'siz.",
  "hero.sub":
    "Discord, Roblox ve oyunları tek tıkla açan Windows aracı. Hızın düşmez — yalnızca seçtiğin sitelerin trafiğine dokunur.",
  "hero.hint": "Kara deliğe dokun ↑",
  "cta.download": "Windows için indir",
  "cta.github": "GitHub'da gör",
  "cta.soon": "v0.1 çok yakında — şimdilik GitHub'dan takip et",
  "platform": "Windows 10 / 11",
  "feat.title": "Ne yapar?",
  "feat.unblock.t": "Engelleri aşar",
  "feat.unblock.d":
    "DPI-bypass motoru: Discord sesi, Roblox ve oyun launcher'ları yeniden açılır. Sansürü kelimenin tam anlamıyla yarıp geçer.",
  "feat.novpn.t": "VPN değil, hız düşmez",
  "feat.novpn.d":
    "Trafiğini başka bir sunucuya yönlendirmez. Yalnızca gereken pakete dokunur; gerisi normal hızında akar.",
  "feat.safe.t": "Cerrahi ve güvenli",
  "feat.safe.d":
    "Anti-cheat farkında (Vanguard / EAC / BattlEye). Her ayar geri alınabilir; riskli olan önce uyarır.",
  "feat.light.t": "Sıfıra yakın kaynak",
  "feat.light.d":
    "Animasyonlu ama hafif: boştayken %0,3'ten az CPU, 120 MB'tan az RAM. Pencere kapanınca tepside çalışır.",
  "how.title": "Nasıl çalışır?",
  "how.1": "İndir ve çalıştır — kurulum derdi yok, hesap yok.",
  "how.2": "Kara deliğe bas — koruma tek tıkla açılır.",
  "how.3": "Oyna — Discord, Roblox ve oyunlar açılır, ping'in korunur.",
  "foot.smartscreen":
    "İmzasız sürüm: ilk açılışta Windows SmartScreen uyarısı çıkabilir → 'More info → Run anyway'.",
  "foot.made": "MIT lisansı altında açık kaynak.",
  "foot.tagline": "VPN olmadan engelleri aş.",
  "nav.faq": "SSS",
  "meta.title": "VPN olmadan engelleri aş",
  "faq.title": "Sık sorulanlar",
  "faq.q1": "Bu bir VPN mi?",
  "faq.a1":
    "Hayır. Trafiğini bir sunucuya yönlendirmez, IP'ni değiştirmez. Yalnızca DPI engelini aşmak için gereken paketlere dokunur; gerisi tam hızında normal akar.",
  "faq.q2": "Oyunlarda / anti-cheat ile güvenli mi?",
  "faq.a2":
    "Evet. Anti-cheat farkındadır (Vanguard / EAC / BattlEye) ve korumalı bir oyun açılınca otomatik duraklayabilir. Her ayar geri alınabilir.",
  "faq.q3": "Ücretsiz mi?",
  "faq.a3": "Evet, tamamen ücretsiz ve açık kaynak (MIT). Hesap yok, abonelik yok.",
  "faq.q4": "Hangi siteleri/uygulamaları açar?",
  "faq.a4":
    "Discord (ses dahil), Roblox, oyun launcher'ları ve listene eklediklerin. Hangi alan adlarının etkileneceğini sen seçersin.",
  "faq.q5": "Windows ilk açılışta uyarı veriyor (SmartScreen)?",
  "faq.a5":
    "Ücretsiz kalmak için sürüm imzasız; bu yüzden Windows bir uyarı gösterebilir → 'More info → Run anyway'. İndirme hash'ini doğrulayabilirsin.",
};

const en = {
  "brand.badge": "Open source · Free · Not a VPN",
  "nav.features": "Features",
  "nav.how": "How it works",
  "nav.github": "GitHub",
  "hero.title": "Break through the block. No VPN.",
  "hero.sub":
    "A Windows tool that unblocks Discord, Roblox and games in one click. No speed loss — it only touches the traffic of the sites you choose.",
  "hero.hint": "Tap the black hole ↑",
  "cta.download": "Download for Windows",
  "cta.github": "View on GitHub",
  "cta.soon": "v0.1 coming very soon — follow on GitHub for now",
  "platform": "Windows 10 / 11",
  "feat.title": "What it does",
  "feat.unblock.t": "Breaks the block",
  "feat.unblock.d":
    "A DPI-bypass engine: Discord voice, Roblox and game launchers come back to life. It literally rifts through censorship.",
  "feat.novpn.t": "Not a VPN, no slowdown",
  "feat.novpn.d":
    "It doesn't route your traffic through another server. It only touches the packets that need it; everything else runs at full speed.",
  "feat.safe.t": "Surgical and safe",
  "feat.safe.d":
    "Anti-cheat aware (Vanguard / EAC / BattlEye). Every setting is reversible; risky ones warn you first.",
  "feat.light.t": "Near-zero footprint",
  "feat.light.d":
    "Animated but light: under 0.3% CPU and 120 MB RAM when idle. Runs in the tray when the window is closed.",
  "how.title": "How it works",
  "how.1": "Download and run — no setup, no account.",
  "how.2": "Hit the black hole — protection turns on in one click.",
  "how.3": "Play — Discord, Roblox and games open up, your ping stays intact.",
  "foot.smartscreen":
    "Unsigned build: Windows SmartScreen may warn on first launch → 'More info → Run anyway'.",
  "foot.made": "Open source under the MIT license.",
  "foot.tagline": "Break through the block. No VPN.",
  "nav.faq": "FAQ",
  "meta.title": "break through the block, no VPN",
  "faq.title": "Frequently asked",
  "faq.q1": "Is this a VPN?",
  "faq.a1":
    "No. It doesn't route your traffic through a server or change your IP. It only touches the packets needed to bypass DPI blocking; everything else flows normally at full speed.",
  "faq.q2": "Is it safe with games / anti-cheat?",
  "faq.a2":
    "Yes. It's anti-cheat aware (Vanguard / EAC / BattlEye) and can auto-pause when a protected game launches. Every setting is reversible.",
  "faq.q3": "Is it free?",
  "faq.a3": "Yes, fully free and open source (MIT). No accounts, no subscriptions.",
  "faq.q4": "Which sites / apps does it unblock?",
  "faq.a4":
    "Discord (including voice), Roblox, game launchers, and whatever you add to the list. You choose which domains are affected.",
  "faq.q5": "Windows warns me on first launch (SmartScreen)?",
  "faq.a5":
    "To stay free the build is unsigned, so Windows may show a SmartScreen prompt → 'More info → Run anyway'. You can verify the download hash.",
};

const es = {
  "brand.badge": "Código abierto · Gratis · No es una VPN",
  "nav.features": "Funciones",
  "nav.how": "Cómo funciona",
  "nav.github": "GitHub",
  "hero.title": "Atraviesa el bloqueo. Sin VPN.",
  "hero.sub":
    "Una herramienta de Windows que desbloquea Discord, Roblox y juegos con un clic. Sin pérdida de velocidad: solo toca el tráfico de los sitios que elijas.",
  "hero.hint": "Toca el agujero negro ↑",
  "cta.download": "Descargar para Windows",
  "cta.github": "Ver en GitHub",
  "cta.soon": "v0.1 muy pronto — sigue en GitHub por ahora",
  "platform": "Windows 10 / 11",
  "feat.title": "Qué hace",
  "feat.unblock.t": "Rompe el bloqueo",
  "feat.unblock.d":
    "Un motor anti-DPI: la voz de Discord, Roblox y los lanzadores de juegos vuelven a funcionar. Atraviesa la censura.",
  "feat.novpn.t": "No es VPN, sin ralentización",
  "feat.novpn.d":
    "No enruta tu tráfico por otro servidor. Solo toca los paquetes necesarios; el resto va a máxima velocidad.",
  "feat.safe.t": "Quirúrgico y seguro",
  "feat.safe.d":
    "Consciente del anti-cheat (Vanguard / EAC / BattlEye). Cada ajuste es reversible; los riesgosos avisan primero.",
  "feat.light.t": "Huella casi nula",
  "feat.light.d":
    "Animado pero ligero: menos del 0,3 % de CPU y 120 MB de RAM en reposo. Funciona en la bandeja al cerrar la ventana.",
  "how.title": "Cómo funciona",
  "how.1": "Descarga y ejecuta — sin instalación, sin cuenta.",
  "how.2": "Pulsa el agujero negro — la protección se activa con un clic.",
  "how.3": "Juega — Discord, Roblox y juegos se abren, tu ping se mantiene.",
  "foot.smartscreen":
    "Build sin firmar: Windows SmartScreen puede avisar al inicio → 'Más información → Ejecutar de todos modos'.",
  "foot.made": "Código abierto bajo licencia MIT.",
  "foot.tagline": "Atraviesa el bloqueo. Sin VPN.",
  "nav.faq": "FAQ",
  "meta.title": "atraviesa el bloqueo, sin VPN",
  "faq.title": "Preguntas frecuentes",
  "faq.q1": "¿Es una VPN?",
  "faq.a1":
    "No. No enruta tu tráfico por un servidor ni cambia tu IP. Solo toca los paquetes necesarios para evitar el bloqueo DPI; el resto fluye normal a máxima velocidad.",
  "faq.q2": "¿Es seguro con juegos / anti-cheat?",
  "faq.a2":
    "Sí. Es consciente del anti-cheat (Vanguard / EAC / BattlEye) y puede pausarse solo cuando se abre un juego protegido. Cada ajuste es reversible.",
  "faq.q3": "¿Es gratis?",
  "faq.a3": "Sí, totalmente gratis y de código abierto (MIT). Sin cuentas, sin suscripciones.",
  "faq.q4": "¿Qué sitios / apps desbloquea?",
  "faq.a4":
    "Discord (incluida la voz), Roblox, lanzadores de juegos y lo que añadas a la lista. Tú eliges qué dominios se ven afectados.",
  "faq.q5": "¿Windows me avisa al iniciar (SmartScreen)?",
  "faq.a5":
    "Para seguir siendo gratis la build no está firmada, así que Windows puede mostrar un aviso de SmartScreen → 'Más información → Ejecutar de todos modos'. Puedes verificar el hash de la descarga.",
};

const ru = {
  "brand.badge": "Открытый код · Бесплатно · Не VPN",
  "nav.features": "Возможности",
  "nav.how": "Как это работает",
  "nav.github": "GitHub",
  "hero.title": "Пробей блокировку. Без VPN.",
  "hero.sub":
    "Программа для Windows, которая разблокирует Discord, Roblox и игры одним кликом. Без потери скорости — затрагивает только трафик выбранных вами сайтов.",
  "hero.hint": "Коснитесь чёрной дыры ↑",
  "cta.download": "Скачать для Windows",
  "cta.github": "Открыть на GitHub",
  "cta.soon": "v0.1 уже совсем скоро — пока следите на GitHub",
  "platform": "Windows 10 / 11",
  "feat.title": "Что она делает",
  "feat.unblock.t": "Снимает блокировку",
  "feat.unblock.d":
    "Движок обхода DPI: голос Discord, Roblox и игровые лаунчеры снова работают. Буквально прорывает цензуру.",
  "feat.novpn.t": "Не VPN, без замедления",
  "feat.novpn.d":
    "Не гонит трафик через чужой сервер. Затрагивает только нужные пакеты; остальное идёт на полной скорости.",
  "feat.safe.t": "Точно и безопасно",
  "feat.safe.d":
    "Учитывает анти-чит (Vanguard / EAC / BattlEye). Любую настройку можно откатить; рискованные предупреждают.",
  "feat.light.t": "Почти нулевая нагрузка",
  "feat.light.d":
    "Анимировано, но легко: меньше 0,3 % CPU и 120 МБ ОЗУ в простое. При закрытии окна работает в трее.",
  "how.title": "Как это работает",
  "how.1": "Скачайте и запустите — без установки и аккаунта.",
  "how.2": "Нажмите на чёрную дыру — защита включается одним кликом.",
  "how.3": "Играйте — Discord, Roblox и игры открываются, пинг сохраняется.",
  "foot.smartscreen":
    "Сборка без подписи: при первом запуске Windows SmartScreen может предупредить → 'Подробнее → Выполнить в любом случае'.",
  "foot.made": "Открытый код под лицензией MIT.",
  "foot.tagline": "Пробей блокировку. Без VPN.",
  "nav.faq": "ЧаВо",
  "meta.title": "пробей блокировку, без VPN",
  "faq.title": "Частые вопросы",
  "faq.q1": "Это VPN?",
  "faq.a1":
    "Нет. Не гонит трафик через сервер и не меняет ваш IP. Затрагивает только пакеты, нужные для обхода DPI; остальное идёт нормально на полной скорости.",
  "faq.q2": "Безопасно с играми / анти-читом?",
  "faq.a2":
    "Да. Учитывает анти-чит (Vanguard / EAC / BattlEye) и может сам приостановиться при запуске защищённой игры. Любую настройку можно откатить.",
  "faq.q3": "Это бесплатно?",
  "faq.a3": "Да, полностью бесплатно и с открытым кодом (MIT). Без аккаунтов и подписок.",
  "faq.q4": "Какие сайты / приложения разблокирует?",
  "faq.a4":
    "Discord (включая голос), Roblox, игровые лаунчеры и всё, что вы добавите в список. Вы сами выбираете, какие домены затрагиваются.",
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
