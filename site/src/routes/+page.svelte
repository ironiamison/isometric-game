<script lang="ts">
  import { onMount } from 'svelte';
  import { appendUtms } from '$lib/utm';

  let navEl: HTMLElement | undefined = $state();
  let starsEl: HTMLElement | undefined = $state();
  let playHref = $state('/play/');

  onMount(() => {
    playHref = appendUtms('/play/');

    if (starsEl) {
      for (let i = 0; i < 60; i++) {
        const star = document.createElement('div');
        star.className = 'star';
        star.style.left = Math.random() * 100 + '%';
        star.style.top = Math.random() * 60 + '%';
        star.style.animationDelay = Math.random() * 3 + 's';
        star.style.animationDuration = 2 + Math.random() * 3 + 's';
        if (Math.random() > 0.7) {
          star.style.width = '4px';
          star.style.height = '4px';
        }
        starsEl.appendChild(star);
      }
    }

    const onScroll = () => {
      navEl?.classList.toggle('visible', window.scrollY > window.innerHeight * 0.4);
    };
    window.addEventListener('scroll', onScroll);

    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            const siblings = entry.target.parentElement?.querySelectorAll('.fade-up') ?? [];
            const idx = Array.from(siblings).indexOf(entry.target);
            setTimeout(() => entry.target.classList.add('visible'), idx * 100);
            observer.unobserve(entry.target);
          }
        });
      },
      { threshold: 0.15 },
    );
    document.querySelectorAll('.fade-up').forEach((el) => observer.observe(el));

    const btn = document.querySelector<HTMLAnchorElement>('.desktop-download-btn');
    const label = document.querySelector('.platform-label');
    const links = document.querySelectorAll<HTMLAnchorElement>('.platform-link');
    if (btn) {
      const downloads = {
        windows: btn.dataset.win,
        mac: btn.dataset.mac,
        linux: btn.dataset.linux,
      };
      const names: Record<string, string> = { windows: 'Windows', mac: 'macOS', linux: 'Linux' };
      const platform = navigator.platform || '';
      const ua = navigator.userAgent || '';
      let detected: keyof typeof downloads | null = null;
      if (/Win/i.test(platform) || /Windows/i.test(ua)) detected = 'windows';
      else if (/Mac/i.test(platform) || /Macintosh/i.test(ua)) detected = 'mac';
      else if (/Linux/i.test(platform) || /X11/i.test(ua)) detected = 'linux';

      if (detected && downloads[detected]) {
        btn.href = downloads[detected]!;
        btn.textContent = `Download (${names[detected]})`;
        if (label) label.textContent = names[detected];
        links.forEach((link) => {
          if (link.dataset.platform === detected) link.classList.add('active');
        });
      } else if (label) {
        label.textContent = 'your platform';
      }
    }

    return () => {
      window.removeEventListener('scroll', onScroll);
      observer.disconnect();
    };
  });
</script>

<svelte:head>
  <title>New Aeven — A Pixel Art Isometric MMO</title>
  <meta
    name="description"
    content="New Aeven is a cozy pixel-art isometric MMO. Farm, craft, explore dungeons, and adventure with friends in a handcrafted open world. Play free in your browser, on desktop, or Android."
  />
  <meta
    name="keywords"
    content="New Aeven, MMO, pixel art, isometric, farming, crafting, dungeons, co-op, free to play, browser game, indie game"
  />
  <meta name="author" content="New Aeven" />
  <meta name="robots" content="index, follow" />
  <link rel="canonical" href="https://aeven.xyz/" />
  <meta property="og:type" content="website" />
  <meta property="og:url" content="https://aeven.xyz/" />
  <meta property="og:title" content="New Aeven — A Pixel Art Isometric MMO" />
  <meta
    property="og:description"
    content="A cozy pixel-art isometric MMO. Farm, craft, explore dungeons, and adventure with friends. Play free in your browser."
  />
  <meta property="og:image" content="https://aeven.xyz/screenshots/screenshot-1.png" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="675" />
  <meta property="og:site_name" content="New Aeven" />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content="New Aeven — A Pixel Art Isometric MMO" />
  <meta
    name="twitter:description"
    content="A cozy pixel-art isometric MMO. Farm, craft, explore dungeons, and adventure with friends. Play free in your browser."
  />
  <meta name="twitter:image" content="https://aeven.xyz/screenshots/screenshot-1.png" />
  <link rel="icon" type="image/png" href="/favicon-96x96.png" sizes="96x96" />
  <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
  <link rel="shortcut icon" href="/favicon.ico" />
  <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png" />
  <meta name="apple-mobile-web-app-title" content="New Aeven" />
  <link rel="manifest" href="/site.webmanifest" />
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
  <link
    href="https://fonts.googleapis.com/css2?family=Silkscreen:wght@400;700&family=Nunito:wght@400;600;700&display=swap"
    rel="stylesheet"
  />
  <link rel="stylesheet" href="/homepage.css" />
  {@html `<script type="application/ld+json">${JSON.stringify({
    '@context': 'https://schema.org',
    '@type': 'VideoGame',
    name: 'New Aeven',
    url: 'https://aeven.xyz',
    description:
      'A cozy pixel-art isometric MMO. Farm, craft, explore dungeons, and adventure with friends in a handcrafted open world.',
    genre: ['MMO', 'RPG', 'Simulation'],
    gamePlatform: ['Web Browser', 'Windows', 'macOS', 'Linux', 'Android'],
    applicationCategory: 'Game',
    operatingSystem: 'Any',
    offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
    image: 'https://aeven.xyz/screenshots/screenshot-1.png',
    screenshot: [
      'https://aeven.xyz/screenshots/screenshot-1.png',
      'https://aeven.xyz/screenshots/screenshot-2.png',
      'https://aeven.xyz/screenshots/screenshot-3.png',
    ],
  })}</script>`}
</svelte:head>

<nav id="nav" bind:this={navEl}>
  <a href="/" class="nav-brand">New Aeven</a>
  <ul class="nav-links">
    <li><a href="#about">About</a></li>
    <li><a href="#play">Play</a></li>
    <li><a href="#community">Community</a></li>
    <li><a href="#media">Media</a></li>
    <li><a href="/world/">World Stats</a></li>
  </ul>
</nav>

<section class="hero">
  <div class="sky"></div>
  <div class="stars" bind:this={starsEl}></div>
  <div class="hero-content">
    <h1 class="game-title">New Aeven</h1>
    <p class="game-subtitle">A cozy pixel-art isometric MMO — craft, explore, and adventure together</p>
    <div class="hero-actions">
      <a href="#play" class="pixel-btn btn-primary">Play Now</a>
      <a href="#community" class="pixel-btn btn-gold">Join Us</a>
    </div>
  </div>
  <div class="scroll-hint">
    <svg viewBox="0 0 24 24"><path d="M12 16l-6-6h12z" /></svg>
  </div>
  <div class="ground">
    <div class="ground-layer ground-back"></div>
    <div class="ground-layer ground-front" style="height: 80px"></div>
    <div class="ground-layer ground-dirt"></div>
  </div>
</section>

<section class="section about" id="about">
  <div class="section-inner fade-up">
    <h2 class="section-title">What is New Aeven?</h2>
    <p class="about-text">
      <strong>New Aeven</strong> is a 2.5D isometric MMO set in a handcrafted pixel-art world. Grow crops on your farm,
      craft gear at your workbench, explore dungeons with friends, or simply hang out in town and trade. No rush, no
      pay-to-win — just a living world waiting for you to make it home.
    </p>
    <div class="feature-chips">
      <span class="chip">Farming</span>
      <span class="chip">Crafting</span>
      <span class="chip">Dungeons</span>
      <span class="chip">Trading</span>
      <span class="chip">Pixel Art</span>
      <span class="chip">Co-op</span>
      <span class="chip">Open World</span>
    </div>
  </div>
</section>

<div class="pixel-divider"></div>

<section class="section cards-section" id="play">
  <div class="section-inner">
    <h2 class="section-title fade-up">Play</h2>
    <div class="cards-grid">
      <div class="card pixel-box fade-up">
        <span class="card-icon">&#x1F310;</span>
        <h3 class="card-title">Play in Browser</h3>
        <p class="card-desc">Jump in instantly — no download needed. Works on any browser.</p>
        <button
          type="button"
          class="pixel-btn btn-water"
          onclick={() => window.location.assign(playHref)}
        >
          Launch
        </button>
      </div>

      <div class="card pixel-box fade-up desktop-download-card" style="position: relative">
        <span class="card-icon">&#x1F5A5;</span>
        <h3 class="card-title">Desktop</h3>
        <p class="card-desc">Download for Windows, macOS, or Linux for the best experience.</p>
        <a
          href="https://dl.aeven.xyz/launcher/new-aeven-launcher-win64.zip"
          class="pixel-btn btn-primary desktop-download-btn"
          data-win="https://dl.aeven.xyz/launcher/new-aeven-launcher-win64.zip"
          data-mac="https://dl.aeven.xyz/launcher/new-aeven-launcher-macos.zip"
          data-linux="https://dl.aeven.xyz/launcher/new-aeven-launcher-linux.tar.gz"
        >
          Download
        </a>
        <div class="platform-hint">Detected: <span class="platform-label">your platform</span>. Not right? Choose below.</div>
        <div class="platform-links">
          <a class="platform-link" data-platform="windows" href="https://dl.aeven.xyz/launcher/new-aeven-launcher-win64.zip">Windows</a>
          <a class="platform-link" data-platform="mac" href="https://dl.aeven.xyz/launcher/new-aeven-launcher-macos.zip">macOS</a>
          <a class="platform-link" data-platform="linux" href="https://dl.aeven.xyz/launcher/new-aeven-launcher-linux.tar.gz">Linux</a>
        </div>
      </div>

      <div class="card pixel-box fade-up">
        <span class="card-icon">&#x1F4F1;</span>
        <h3 class="card-title">Android</h3>
        <p class="card-desc">Take Aeven with you. Available on Android devices.</p>
        <a href="https://discord.gg/VHB9qSyhUF" class="pixel-btn btn-ember">Get APK</a>
      </div>
    </div>
  </div>
</section>

<div class="divider-diamond">
  <span class="diamond"></span>
  <span class="diamond"></span>
  <span class="diamond"></span>
</div>

<section class="section about" id="community">
  <div class="section-inner">
    <h2 class="section-title fade-up">Community</h2>
    <p class="about-text fade-up" style="margin-bottom: 1.5rem">
      New Aeven is built alongside its community. Come say hello, share your adventures, report bugs, or just hang out.
    </p>
    <div class="hero-actions fade-up">
      <a href="https://discord.gg/VHB9qSyhUF" class="pixel-btn btn-primary" style="background: #5865f2">Discord</a>
      <a href="https://discord.gg/VHB9qSyhUF" class="pixel-btn btn-gold">Wiki</a>
      <a href="/world/" class="pixel-btn btn-water">World Stats</a>
    </div>
  </div>
</section>

<div class="pixel-divider"></div>

<section class="section cards-section" id="media">
  <div class="section-inner">
    <h2 class="section-title fade-up" style="color: var(--parchment)">Media</h2>
    <div class="media-grid">
      {#each [1, 2, 3] as n}
        <div class="pixel-box fade-up media-screenshot" style="overflow: hidden; aspect-ratio: 16/9">
          <img
            src="/screenshots/screenshot-{n}.png"
            alt="Screenshot {n} of New Aeven gameplay"
            style="width: 100%; height: 100%; object-fit: cover; display: block; border-radius: 6px"
          />
        </div>
      {/each}
    </div>
  </div>
</section>

<footer>
  <p>New Aeven &mdash; made with care</p>
  <p class="footer-pixel">* * *</p>
</footer>
