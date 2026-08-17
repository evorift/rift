<script>
  import Shell from "$lib/Shell.svelte";
  import { getLang } from "$lib/i18n.svelte.js";

  const SPONSOR = "https://github.com/sponsors/evorift";
  const REPO = "https://github.com/evorift/rift";

  const L = {
    tr: {
      title: "Destek ol",
      lead: "evorift ücretsiz ve öyle kalacak. Destek isteğe bağlı — vermezsen hiçbir şey eksilmez.",
      whyT: "Neye gidiyor",
      whyD:
        "Sunucu maliyetimiz yok, çünkü sunucumuz yok. Destek, geliştirmeye ayrılan zamana ve ileride kod imzalama sertifikasına gider — sertifika alınırsa SmartScreen uyarısı ortadan kalkar.",
      cta: "GitHub Sponsors",
      freeT: "Para vermeden de destek olur",
      freeList: [
        "Çalışmayan bir durumu ayrıntısıyla bildirmek — hangi operatör, hangi mod, ne oldu.",
        "Çalıştığını bildirmek. Hangi hatta neyin işe yaradığı, en çok ihtiyaç duyulan bilgi.",
        "Kodu okuyup hata bulmak.",
      ],
      noteT: "Söz vermediğimiz şey",
      noteD:
        "Destek olmak öncelikli yanıt, özel özellik ya da garanti getirmez. Böyle bir katman kurmuyoruz; kurarsak da ücretsiz sürümden bir şey eksilterek olmayacak.",
    },
    en: {
      title: "Support",
      lead: "evorift is free and will stay that way. Supporting it is optional — nothing is withheld if you don't.",
      whyT: "Where it goes",
      whyD:
        "We have no server costs, because we run no servers. Support goes to development time and, later, a code-signing certificate — if we get one, the SmartScreen warning goes away.",
      cta: "GitHub Sponsors",
      freeT: "You can help without paying",
      freeList: [
        "Report something that doesn't work, in detail — which provider, which mode, what happened.",
        "Report that it does work. Which setting helped on which line is the information we need most.",
        "Read the code and find bugs.",
      ],
      noteT: "What we are not promising",
      noteD:
        "Sponsoring does not buy priority replies, private features or guarantees. We are not building that tier; if we ever do, it will not be by taking something away from the free build.",
    },
  };

  const c = $derived(L[getLang()] ?? L.en);
</script>

<Shell title={c.title} lead={c.lead}>
  <section class="block">
    <h2>{c.whyT}</h2>
    <p>{c.whyD}</p>
    <p><a class="btn" href={SPONSOR} target="_blank" rel="noopener">♥ {c.cta}</a></p>
  </section>

  <section class="block">
    <h2>{c.freeT}</h2>
    <ul class="list">
      {#each c.freeList as i (i)}<li>{i}</li>{/each}
    </ul>
    <p><a href={REPO} target="_blank" rel="noopener">github.com/evorift/rift</a></p>
  </section>

  <section class="block">
    <h2>{c.noteT}</h2>
    <p>{c.noteD}</p>
  </section>
</Shell>

<style>
  .block { margin-top: 36px; }
  .block h2 { font-size: 20px; font-weight: 800; letter-spacing: -0.01em; margin-bottom: 10px; }
  .block p { color: var(--text-muted); line-height: 1.65; margin-bottom: 10px; }
  .list { margin: 0 0 10px; padding-left: 20px; color: var(--text-muted); line-height: 1.8; }
</style>
