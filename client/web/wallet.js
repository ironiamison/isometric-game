// Phantom wallet sign-in for Solstead (browser WASM)
"use strict";

const WALLET_PLUGIN = {
  pending: new Map(),
  next_id: 1,
};

function bytesToBase64(bytes) {
  let binary = "";
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

miniquad_add_plugin({
  name: "wallet",
  version: "0.1.0",
  register_plugin: function (importObject) {
    importObject.env.wallet_is_available = function () {
      return window.solana && window.solana.isPhantom ? 1 : 0;
    };

    importObject.env.wallet_sign_start = function (message_js) {
      const message = consume_js_object(message_js);
      const request_id = WALLET_PLUGIN.next_id++;
      const entry = { status: 0, result: null, error: null };
      WALLET_PLUGIN.pending.set(request_id, entry);

      (async function () {
        try {
          const provider = window.solana;
          if (!provider || !provider.isPhantom) {
            throw new Error("Phantom wallet not found");
          }
          const resp = await provider.connect();
          const pubkey = resp.publicKey.toBase58();
          const encoded = new TextEncoder().encode(message);
          const signed = await provider.signMessage(encoded, "utf8");
          entry.result = JSON.stringify({
            pubkey: pubkey,
            signature: bytesToBase64(signed.signature),
          });
          entry.status = 1;
        } catch (err) {
          console.error("wallet_sign error:", err);
          entry.error = err && err.message ? err.message : String(err);
          entry.status = 2;
        }
      })();

      return request_id;
    };

    importObject.env.wallet_sign_poll = function (request_id) {
      const entry = WALLET_PLUGIN.pending.get(request_id);
      if (!entry) return 2;
      return entry.status;
    };

    importObject.env.wallet_sign_result = function (request_id) {
      const entry = WALLET_PLUGIN.pending.get(request_id);
      if (!entry || entry.status !== 1 || !entry.result) return -1;
      return js_object(entry.result);
    };

    importObject.env.wallet_sign_error = function (request_id) {
      const entry = WALLET_PLUGIN.pending.get(request_id);
      if (!entry || !entry.error) return -1;
      return js_object(entry.error);
    };

    importObject.env.wallet_sign_cleanup = function (request_id) {
      WALLET_PLUGIN.pending.delete(request_id);
    };
  },
});
