// Title screen / homescreen ambient music (menu.ogg)
"use strict";

const SolsteadTitleAudio = (function () {
  const STORAGE_KEY = "solstead_music_muted";
  const MUSIC_SRC = "assets/audio/menu.ogg";
  const DEFAULT_VOLUME = 0.38;

  let audio = null;
  let unlocked = false;

  function getAudio() {
    if (!audio) {
      audio = new Audio(MUSIC_SRC);
      audio.loop = true;
      audio.volume = DEFAULT_VOLUME;
      audio.preload = "auto";
    }
    return audio;
  }

  function isMuted() {
    try {
      return localStorage.getItem(STORAGE_KEY) === "1";
    } catch (_) {
      return false;
    }
  }

  function setMuted(muted) {
    try {
      localStorage.setItem(STORAGE_KEY, muted ? "1" : "0");
    } catch (_) {}
    updateButton();
    const track = getAudio();
    if (muted) {
      track.pause();
      return;
    }
    play();
  }

  function play() {
    if (isMuted()) return;
    getAudio().play().catch(function () {});
  }

  function unlockAndPlay() {
    if (unlocked) return;
    unlocked = true;
    play();
    updateButton();
  }

  function updateButton() {
    const btn = document.getElementById("title-music-toggle");
    if (!btn) return;
    const muted = isMuted();
    const playing = unlocked && !muted && audio && !audio.paused;
    btn.classList.toggle("is-playing", playing);
    btn.classList.toggle("is-muted", muted);
    btn.setAttribute("aria-pressed", playing ? "true" : "false");
    btn.title = muted ? "Unmute music" : "Mute music";
  }

  function bindUnlock() {
    const unlock = function () {
      unlockAndPlay();
    };
    document.addEventListener("pointerdown", unlock, { once: true });
    document.addEventListener("keydown", unlock, { once: true });
  }

  function init() {
    const btn = document.getElementById("title-music-toggle");
    if (btn) {
      btn.addEventListener("click", function (event) {
        event.stopPropagation();
        unlockAndPlay();
        setMuted(!isMuted());
      });
    }
    bindUnlock();
    updateButton();
    if (!isMuted()) {
      getAudio().addEventListener("playing", updateButton);
      getAudio().addEventListener("pause", updateButton);
    }
  }

  function stop() {
    if (audio) audio.pause();
  }

  return { init, stop, play, setMuted, isMuted };
})();
