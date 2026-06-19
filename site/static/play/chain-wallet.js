// Solstead devnet SPL wallet panel — deposit / withdraw SOLST tokens
"use strict";

const SolsteadChainWallet = (function () {
  const TOKEN_PROGRAM_ID = new solanaWeb3.PublicKey(
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
  );
  const ASSOCIATED_TOKEN_PROGRAM_ID = new solanaWeb3.PublicKey(
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
  );

  function anchorDiscriminator(name) {
    const preimage = new TextEncoder().encode("global:" + name);
    return window.crypto.subtle.digest("SHA-256", preimage).then(function (hash) {
      return new Uint8Array(hash).slice(0, 8);
    });
  }

  function apiGet(url, token) {
    return fetch(url, {
      headers: token ? { Authorization: "Bearer " + token } : {},
    }).then(function (r) {
      if (!r.ok) throw new Error("HTTP " + r.status);
      return r.json();
    });
  }

  function apiPost(url, body, token) {
    return fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: "Bearer " + token,
      },
      body: JSON.stringify(body),
    }).then(function (r) {
      return r.json();
    });
  }

  function getAssociatedTokenAddress(mint, owner) {
    return solanaWeb3.PublicKey.findProgramAddressSync(
      [
        owner.toBuffer(),
        TOKEN_PROGRAM_ID.toBuffer(),
        mint.toBuffer(),
      ],
      ASSOCIATED_TOKEN_PROGRAM_ID
    )[0];
  }

  function deriveVault(programId, mint) {
    return solanaWeb3.PublicKey.findProgramAddressSync(
      [new TextEncoder().encode("vault"), mint.toBuffer()],
      programId
    );
  }

  async function buildDepositTransaction(connection, config, walletPubkey, amountUi) {
    const programId = new solanaWeb3.PublicKey(config.program_id);
    const mint = new solanaWeb3.PublicKey(config.mint);
    const depositor = new solanaWeb3.PublicKey(walletPubkey);
    const [vault] = deriveVault(programId, mint);
    const depositorAta = getAssociatedTokenAddress(mint, depositor);
    const vaultAta = getAssociatedTokenAddress(mint, vault);
    const amount = BigInt(Math.round(amountUi * Math.pow(10, config.token_decimals)));

    const disc = await anchorDiscriminator("deposit");
    const data = new Uint8Array(16);
    data.set(disc, 0);
    const view = new DataView(data.buffer);
    view.setBigUint64(8, amount, true);

    const ix = new solanaWeb3.TransactionInstruction({
      programId,
      keys: [
        { pubkey: depositor, isSigner: true, isWritable: true },
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: vault, isSigner: false, isWritable: false },
        { pubkey: depositorAta, isSigner: false, isWritable: true },
        { pubkey: vaultAta, isSigner: false, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      ],
      data,
    });

    const tx = new solanaWeb3.Transaction().add(ix);
    tx.feePayer = depositor;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    return tx;
  }

  function ensurePanel() {
    let panel = document.getElementById("chain-wallet-panel");
    if (panel) return panel;

    panel = document.createElement("section");
    panel.id = "chain-wallet-panel";
    panel.className = "chain-wallet-panel";
    panel.innerHTML =
      '<h3>SOLST Wallet <span class="chain-tag">Devnet</span></h3>' +
      '<p class="chain-balance">Balance: <strong id="chain-balance">—</strong> SOLST</p>' +
      '<div class="chain-actions">' +
      '<input type="number" id="chain-amount" min="0.000001" step="0.1" value="1" />' +
      '<button type="button" id="chain-deposit-btn" class="ss-btn-outline">Deposit</button>' +
      '<button type="button" id="chain-withdraw-btn" class="ss-btn-outline">Withdraw</button>' +
      "</div>" +
      '<p id="chain-status" class="chain-status"></p>';

    const hero = document.querySelector(".title-hero-stack");
    if (hero) hero.appendChild(panel);
    return panel;
  }

  async function refresh(serverUrl, session) {
    const config = await apiGet(serverUrl + "/api/chain/config");
    if (!config.enabled) return null;

    const balance = await apiGet(serverUrl + "/api/chain/balance", session.token);
    const el = document.getElementById("chain-balance");
    if (el && balance.success) {
      el.textContent = String(balance.balance ?? 0);
    }
    return config;
  }

  async function mount(serverUrl, session) {
    if (!session || !session.token) return;

    const config = await apiGet(serverUrl + "/api/chain/config");
    if (!config.enabled) return;

    ensurePanel();
    await refresh(serverUrl, session);

    document.getElementById("chain-deposit-btn").onclick = async function () {
      const status = document.getElementById("chain-status");
      status.textContent = "Preparing deposit…";
      try {
        const provider = window.solana;
        if (!provider || !provider.isPhantom) throw new Error("Phantom required");
        const amount = parseFloat(document.getElementById("chain-amount").value);
        if (!(amount > 0)) throw new Error("Enter a valid amount");

        const walletResp = await provider.connect();
        const walletPubkey = walletResp.publicKey.toBase58();
        const connection = new solanaWeb3.Connection("https://api.devnet.solana.com");
        const tx = await buildDepositTransaction(connection, config, walletPubkey, amount);
        const signed = await provider.signTransaction(tx);
        const sig = await connection.sendRawTransaction(signed.serialize());
        await connection.confirmTransaction(sig, "confirmed");
        status.textContent = "Deposit sent: " + sig.slice(0, 8) + "… (indexer credits in ~30s)";
        setTimeout(function () {
          refresh(serverUrl, session);
        }, 15000);
      } catch (err) {
        status.textContent = err.message || String(err);
      }
    };

    document.getElementById("chain-withdraw-btn").onclick = async function () {
      const status = document.getElementById("chain-status");
      status.textContent = "Submitting withdraw…";
      try {
        const amount = parseFloat(document.getElementById("chain-amount").value);
        const resp = await apiPost(serverUrl + "/api/chain/withdraw", { amount }, session.token);
        if (!resp.success) throw new Error(resp.error || "Withdraw failed");
        status.textContent = "Withdraw confirmed: " + (resp.tx_signature || "").slice(0, 8) + "…";
        await refresh(serverUrl, session);
      } catch (err) {
        status.textContent = err.message || String(err);
      }
    };
  }

  return { mount, refresh };
})();
