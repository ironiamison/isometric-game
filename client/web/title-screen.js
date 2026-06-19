// Solstead title screen — wallet / guest auth, then enter game
"use strict";

const SolsteadTitleScreen = (function () {
  const SESSION_KEY = "solstead_auth_session";

  let session = null;
  let entered = false;
  let walletBusy = false;
  let serverUrl = null;

  const CONNECT_IDS = ["title-connect-wallet"];
  const GUEST_IDS = ["title-guest-header"];
  const ONLINE_BASELINE = 6;

  function refreshOnlineCount() {
    const countEl = document.getElementById("title-online-count");
    if (!countEl) return;
    const statsUrl = (siteRoot() || "") + "/api/stats/overview";
    fetch(statsUrl)
      .then(function (r) {
        return r.ok ? r.json() : null;
      })
      .then(function (data) {
        const n =
          data && typeof data.online_players === "number" && data.online_players > 0
            ? data.online_players
            : ONLINE_BASELINE;
        countEl.textContent = String(n);
      })
      .catch(function () {
        countEl.textContent = String(ONLINE_BASELINE);
      });
  }

  function resolveServerUrl() {
    const origin = window.location.origin;
    if (origin.includes("localhost") || origin.includes("127.0.0.1")) {
      return "http://localhost:2567";
    }
    return origin.replace(/\/$/, "");
  }

  function siteRoot() {
    const path = window.location.pathname;
    if (path.includes("/play")) {
      return path.split("/play")[0] || "";
    }
    return "";
  }

  function setStatus(msg, isError) {
    const el = document.getElementById("title-wallet-status");
    if (!el) return;
    el.textContent = msg || "";
    el.classList.remove("error", "ok");
    if (isError) el.classList.add("error");
    else if (msg && /connected|signed in|ready/i.test(msg)) el.classList.add("ok");
  }

  function setButtonsEnabled(canEnter) {
    const enterBtn = document.getElementById("title-enter-btn");
    if (enterBtn) {
      enterBtn.disabled = !canEnter;
      enterBtn.classList.toggle("ready", canEnter);
    }
    CONNECT_IDS.forEach(function (id) {
      const btn = document.getElementById(id);
      if (btn) btn.disabled = walletBusy;
    });
    GUEST_IDS.forEach(function (id) {
      const btn = document.getElementById(id);
      if (btn) btn.disabled = walletBusy;
    });
  }

  function updateHeaderAuth() {
    const loggedIn = !!(session && session.token && session.username);
    CONNECT_IDS.forEach(function (id) {
      const btn = document.getElementById(id);
      if (btn) btn.hidden = loggedIn;
    });
    GUEST_IDS.forEach(function (id) {
      const btn = document.getElementById(id);
      if (btn) btn.hidden = loggedIn;
    });
    const userEl = document.getElementById("title-user");
    if (userEl) {
      userEl.hidden = !loggedIn;
      userEl.textContent = loggedIn ? session.username : "";
      userEl.title = loggedIn ? session.username : "";
    }
    const signOutBtn = document.getElementById("title-sign-out");
    if (signOutBtn) signOutBtn.hidden = !loggedIn;
  }

  function signOut() {
    session = null;
    try {
      localStorage.removeItem(SESSION_KEY);
      localStorage.removeItem("remembered_username");
    } catch (e) {
      console.error("storage:", e);
    }
    updateHeaderAuth();
    setStatus("", false);
    setButtonsEnabled(false);
  }

  function saveSession(data) {
    session = {
      token: data.token,
      username: data.username,
      characters: data.characters || [],
    };
    try {
      localStorage.setItem(SESSION_KEY, JSON.stringify(session));
      localStorage.setItem("remembered_username", session.username);
    } catch (e) {
      console.error("storage:", e);
    }
    setStatus("Signed in — click Enter Solstead", false);
    setButtonsEnabled(true);
    updateHeaderAuth();
  }

  function parseJsonResponse(text, res) {
    if (!text || !text.trim()) {
      throw new Error(
        res.ok ? "Empty server response" : "Server unavailable (" + res.status + ")"
      );
    }
    try {
      return JSON.parse(text);
    } catch (_) {
      throw new Error(
        res.ok ? "Invalid server response" : "Server unavailable (" + res.status + ")"
      );
    }
  }

  async function apiPost(path, body) {
    const res = await fetch(serverUrl + path, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body || {}),
    });
    const text = await res.text();
    const data = parseJsonResponse(text, res);
    if (!data.success) {
      throw new Error(data.error || "Request failed");
    }
    return data;
  }

  async function apiGet(path) {
    const res = await fetch(serverUrl + path);
    const text = await res.text();
    return parseJsonResponse(text, res);
  }

  function bytesToBase64(bytes) {
    let binary = "";
    for (let i = 0; i < bytes.length; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
  }

  async function connectWallet() {
    if (walletBusy) return;
    walletBusy = true;
    setButtonsEnabled(false);
    setStatus("Connecting wallet…", false);

    try {
      const provider = window.solana;
      if (!provider || !provider.isPhantom) {
        throw new Error("Install Phantom wallet, or use Play as Guest.");
      }

      const resp = await provider.connect();
      const pubkey = resp.publicKey.toBase58();

      setStatus("Sign the message in Phantom…", false);
      const challenge = await apiGet("/api/wallet/challenge");
      if (!challenge.nonce || !challenge.message) {
        throw new Error("Could not get wallet challenge");
      }

      const encoded = new TextEncoder().encode(challenge.message);
      const signed = await provider.signMessage(encoded, "utf8");

      setStatus("Verifying…", false);
      const login = await apiPost("/api/wallet/login", {
        pubkey: pubkey,
        signature: bytesToBase64(signed.signature),
        nonce: challenge.nonce,
      });

      saveSession(login);
    } catch (err) {
      console.error("wallet connect:", err);
      setStatus(err.message || String(err), true);
      setButtonsEnabled(!!session);
    } finally {
      walletBusy = false;
      setButtonsEnabled(!!session);
    }
  }

  async function playAsGuest() {
    if (walletBusy) return;
    walletBusy = true;
    setStatus("Creating guest account…", false);
    setButtonsEnabled(false);

    try {
      const data = await apiPost("/api/guest", {});
      saveSession(data);
    } catch (err) {
      console.error("guest:", err);
      setStatus(err.message || String(err), true);
      setButtonsEnabled(false);
    } finally {
      walletBusy = false;
      if (session) setButtonsEnabled(true);
    }
  }

  function enterGame() {
    if (!session || entered) return;
    if (window.SolsteadTitleAudio) window.SolsteadTitleAudio.stop();
    entered = true;

    try {
      localStorage.setItem(SESSION_KEY, JSON.stringify(session));
      localStorage.setItem("remembered_username", session.username);
    } catch (e) {
      console.error("storage:", e);
    }

    const title = document.getElementById("title-screen");
    const canvas = document.getElementById("glcanvas");
    const loading = document.getElementById("loading-overlay");

    if (loading) {
      loading.style.display = "flex";
      loading.classList.remove("hidden");
    }
    if (canvas) canvas.style.visibility = "visible";

    if (typeof load === "function") {
      load("isometric_client.wasm");
      setTimeout(function () {
        if (canvas) {
          canvas.setAttribute("tabindex", "1");
          canvas.focus();
        }
      }, 600);
    }

    if (title) {
      title.classList.add("hidden");
      setTimeout(function () {
        title.style.display = "none";
      }, 400);
    }
  }

  function shortenAddress(address, head, tail) {
    head = head || 6;
    tail = tail || 6;
    if (!address || address.length <= head + tail + 3) return address || "";
    return address.slice(0, head) + "…" + address.slice(-tail);
  }

  function initContractAddress() {
    const cfg = window.SOLSTEAD_CHAIN || {};
    const mint = cfg.tokenMint;
    if (!mint) return;

    const addressEl = document.getElementById("title-contract-address");
    const symbolEl = document.getElementById("title-contract-symbol");
    const clusterEl = document.getElementById("title-contract-cluster");
    const copyBtn = document.getElementById("title-contract-copy");
    if (!addressEl) return;

    addressEl.textContent = mint;
    addressEl.title = mint;
    if (symbolEl && cfg.tokenSymbol) symbolEl.textContent = cfg.tokenSymbol;
    if (clusterEl && cfg.cluster) clusterEl.textContent = cfg.cluster;

    if (copyBtn) {
      copyBtn.addEventListener("click", function () {
        navigator.clipboard.writeText(mint).then(
          function () {
            const label = copyBtn.querySelector(".title-contract-copy-label");
            if (label) label.textContent = "Copied";
            else copyBtn.textContent = "Copied";
            setTimeout(function () {
              if (label) label.textContent = "Copy";
              else copyBtn.textContent = "Copy";
            }, 2000);
          },
          function () {}
        );
      });
    }
  }

  function initAudio() {
    if (window.SolsteadTitleAudio) {
      window.SolsteadTitleAudio.init();
    }
  }

  function restoreSession() {
    try {
      const raw = localStorage.getItem(SESSION_KEY);
      if (!raw) return;
      const data = JSON.parse(raw);
      if (data && data.token && data.username) {
        saveSession(data);
      }
    } catch (e) {
      console.error("restore session:", e);
    }
  }

  function bindEvents() {
    CONNECT_IDS.forEach(function (id) {
      const el = document.getElementById(id);
      if (el) el.addEventListener("click", connectWallet);
    });
    GUEST_IDS.forEach(function (id) {
      const el = document.getElementById(id);
      if (el) el.addEventListener("click", playAsGuest);
    });

    const enter = document.getElementById("title-enter-btn");
    if (enter) enter.addEventListener("click", enterGame);

    const signOutBtn = document.getElementById("title-sign-out");
    if (signOutBtn) signOutBtn.addEventListener("click", signOut);
  }

  function init() {
    serverUrl = resolveServerUrl();
    const root = siteRoot();
    const canvas = document.getElementById("glcanvas");
    if (canvas) canvas.style.visibility = "hidden";

    document.querySelectorAll("[data-site-href]").forEach(function (el) {
      el.setAttribute("href", root + el.getAttribute("data-site-href"));
    });

    bindEvents();
    restoreSession();
    initContractAddress();
    initAudio();
    if (!session) {
      setButtonsEnabled(false);
      setStatus("", false);
    }
    updateHeaderAuth();
    refreshOnlineCount();
    setInterval(refreshOnlineCount, 15000);
  }

  return {
    init: init,
    isEntered: function () {
      return entered;
    },
  };
})();

window.SolsteadTitleScreen = SolsteadTitleScreen;

document.addEventListener("DOMContentLoaded", function () {
  SolsteadTitleScreen.init();
});
