<script lang="ts">
  import { onMount } from 'svelte';

  const MUSIC_SRC = '/play/assets/audio/menu.ogg';
  const STORAGE_KEY = 'solstead_music_muted';

  let audio: HTMLAudioElement | null = $state(null);
  let unlocked = $state(false);
  let muted = $state(false);
  let playing = $state(false);

  onMount(() => {
    try {
      muted = localStorage.getItem(STORAGE_KEY) === '1';
    } catch {
      muted = false;
    }

    audio = new Audio(MUSIC_SRC);
    audio.loop = true;
    audio.volume = 0.38;
    audio.preload = 'auto';

    const unlock = () => {
      if (unlocked) return;
      unlocked = true;
      if (!muted) void play();
    };

    window.addEventListener('pointerdown', unlock, { once: true });
    window.addEventListener('keydown', unlock, { once: true });

    return () => {
      audio?.pause();
    };
  });

  async function play() {
    if (!audio || muted) return;
    try {
      await audio.play();
      playing = true;
    } catch {
      playing = false;
    }
  }

  function toggleMusic() {
    unlocked = true;
    muted = !muted;
    try {
      localStorage.setItem(STORAGE_KEY, muted ? '1' : '0');
    } catch {
      // ignore
    }
    if (muted) {
      audio?.pause();
      playing = false;
    } else {
      void play();
    }
  }
</script>

<button
  type="button"
  class="home-music-toggle"
  class:is-playing={playing}
  class:is-muted={muted}
  aria-label="Toggle music"
  aria-pressed={playing}
  title={muted ? 'Unmute music' : 'Mute music'}
  onclick={toggleMusic}
>
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6z" />
  </svg>
</button>

<style>
  .home-music-toggle {
    position: fixed;
    top: 1rem;
    right: 1rem;
    z-index: 200;
    width: 42px;
    height: 42px;
    border: 2px solid var(--gold);
    border-radius: 999px;
    background: rgba(42, 26, 58, 0.82);
    color: var(--gold-light);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35);
  }

  .home-music-toggle svg {
    width: 18px;
    height: 18px;
    fill: currentColor;
  }

  .home-music-toggle.is-playing {
    box-shadow: 0 0 18px rgba(212, 168, 68, 0.35);
  }

  .home-music-toggle.is-muted {
    opacity: 0.55;
  }
</style>
