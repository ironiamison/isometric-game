<script lang="ts">
  import {
    SOLSTEAD_CHAIN_CLUSTER,
    SOLSTEAD_TOKEN_MINT,
    SOLSTEAD_TOKEN_SYMBOL,
  } from '$lib/chain-config';

  let copied = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | undefined;

  async function copyAddress() {
    try {
      await navigator.clipboard.writeText(SOLSTEAD_TOKEN_MINT);
      copied = true;
      clearTimeout(copyTimer);
      copyTimer = setTimeout(() => {
        copied = false;
      }, 2000);
    } catch {
      // Clipboard may be blocked.
    }
  }
</script>

<div class="contract-card" aria-label="Token contract address">
  <div class="contract-card-frame">
    <div class="contract-card-inner">
      <div class="contract-card-head">
        <div class="contract-card-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24">
            <path
              d="M10.59 13.41c.41.39.41 1.03 0 1.42-.39.39-1.03.39-1.42 0a5.003 5.003 0 0 1 0-7.07l3.54-3.54a5.003 5.003 0 0 1 7.07 0 5.003 5.003 0 0 1 0 7.07l-1.49 1.49c.01-.82-.12-1.64-.4-2.43l.47-.48a3.003 3.003 0 0 0 0-4.24 3.003 3.003 0 0 0-4.24 0l-3.53 3.53a3.003 3.003 0 0 0 0 4.24zm2.82-4.24c.39-.39 1.03-.39 1.42 0a5.003 5.003 0 0 1 0 7.07l-3.54 3.54a5.003 5.003 0 0 1-7.07 0 5.003 5.003 0 0 1 0-7.07l1.49-1.49c-.01.82.12 1.64.4 2.43l-.47.48a3.003 3.003 0 0 0 0 4.24 3.003 3.003 0 0 0 4.24 0l3.53-3.53a3.003 3.003 0 0 0 0-4.24.973.973 0 0 1 0-1.42z"
            />
          </svg>
        </div>
        <div class="contract-card-copyblock">
          <span class="contract-card-label pixel-font">Token Contract</span>
          <span class="contract-card-hint">Official on-chain mint address</span>
        </div>
        <div class="contract-card-badges">
          <span class="contract-card-badge">{SOLSTEAD_TOKEN_SYMBOL}</span>
          <span class="contract-card-badge is-cluster">{SOLSTEAD_CHAIN_CLUSTER}</span>
        </div>
      </div>
      <div class="contract-card-row">
        <code class="contract-card-value" title={SOLSTEAD_TOKEN_MINT}>
          {SOLSTEAD_TOKEN_MINT}
        </code>
        <button type="button" class="contract-card-copy pixel-font" onclick={copyAddress}>
          {copied ? 'Copied' : 'Copy'}
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .contract-card {
    width: min(100%, 36rem);
    margin: 1.75rem auto 0;
  }

  .contract-card-frame {
    position: relative;
    padding: 1px;
    border-radius: 12px;
    background: linear-gradient(
      135deg,
      rgba(232, 200, 74, 0.65) 0%,
      rgba(138, 112, 32, 0.28) 45%,
      rgba(232, 200, 74, 0.5) 100%
    );
    box-shadow:
      0 10px 28px rgba(0, 0, 0, 0.42),
      inset 0 1px 0 rgba(255, 255, 255, 0.08);
  }

  .contract-card-frame::before,
  .contract-card-frame::after {
    content: '';
    position: absolute;
    width: 8px;
    height: 8px;
    background: var(--gold-light);
    transform: rotate(45deg);
    box-shadow: 0 0 8px rgba(212, 168, 68, 0.45);
    z-index: 2;
  }

  .contract-card-frame::before {
    top: 10px;
    left: 10px;
  }

  .contract-card-frame::after {
    top: 10px;
    right: 10px;
  }

  .contract-card-inner {
    border-radius: 11px;
    padding: 1rem 1.1rem 1.05rem;
    background: linear-gradient(180deg, rgba(24, 16, 10, 0.94) 0%, rgba(10, 8, 6, 0.98) 100%);
    border: 1px solid rgba(212, 168, 68, 0.18);
  }

  .contract-card-head {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    margin-bottom: 0.85rem;
  }

  .contract-card-icon {
    width: 34px;
    height: 34px;
    border: 1px solid rgba(212, 168, 68, 0.35);
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(212, 168, 68, 0.08);
    flex-shrink: 0;
  }

  .contract-card-icon svg {
    width: 18px;
    height: 18px;
    fill: var(--gold-light);
  }

  .contract-card-copyblock {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    flex: 1;
    min-width: 0;
    text-align: left;
  }

  .contract-card-label {
    font-size: 0.82rem;
    letter-spacing: 0.06em;
    color: #f0d878;
  }

  .contract-card-hint {
    font-size: 0.68rem;
    color: rgba(245, 230, 200, 0.62);
  }

  .contract-card-badges {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    align-items: flex-end;
    flex-shrink: 0;
  }

  .contract-card-badge {
    font-size: 0.58rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    padding: 0.22rem 0.5rem;
    border: 1px solid rgba(232, 200, 74, 0.45);
    color: #f0d878;
    background: rgba(212, 168, 68, 0.1);
  }

  .contract-card-badge.is-cluster {
    color: rgba(245, 230, 200, 0.72);
    border-color: rgba(212, 168, 68, 0.28);
    background: rgba(0, 0, 0, 0.18);
  }

  .contract-card-row {
    display: flex;
    align-items: stretch;
    gap: 0.55rem;
  }

  .contract-card-value {
    flex: 1;
    min-width: 0;
    display: block;
    padding: 0.72rem 0.85rem;
    border-radius: 8px;
    border: 1px solid rgba(212, 168, 68, 0.22);
    background: rgba(0, 0, 0, 0.35);
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 0.84rem;
    line-height: 1.35;
    color: #f3df8d;
    text-align: left;
    word-break: break-all;
  }

  .contract-card-copy {
    flex-shrink: 0;
    min-width: 74px;
    padding: 0 0.9rem;
    border: 2px solid var(--gold);
    border-radius: 8px;
    background: rgba(212, 168, 68, 0.12);
    color: var(--gold-light);
    font-size: 0.68rem;
    cursor: pointer;
  }

  .contract-card-copy:hover {
    background: rgba(212, 168, 68, 0.22);
  }
</style>
