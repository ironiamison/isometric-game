<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { api, type Entity } from '$lib/api';
  import { formatChance, formatItemName, signed } from '$lib/format';
  import { ArrowLeft, Dice5, Droplets, ScrollText, Skull, TrendingUp } from 'lucide-svelte';

  let entities: Entity[] | undefined = $state();
  let isLoading = $state(true);

  let monsterId = $derived($page.params.id ?? '');
  let monster = $derived(entities?.find((e) => e.id === monsterId));

  let scalingRows = $derived.by(() => {
    if (!monster) return [];
    const maxLevel = Math.max(monster.level, 20);
    return Array.from({ length: maxLevel }, (_, i) => {
      const level = i + 1;
      return {
        level,
        hp: Math.round(monster.max_hp * (1 + 0.1 * Math.max(0, level - 1))),
        damage: Math.round(monster.damage * (1 + 0.15 * Math.max(0, level - 1))),
        exp: monster.exp_base * level,
        goldMin: monster.gold_min * level,
        goldMax: monster.gold_max * level,
      };
    });
  });

  function formatRespawn(ms: number) {
    const seconds = ms / 1000;
    if (seconds >= 60) {
      const m = Math.floor(seconds / 60);
      const s = seconds % 60;
      return s > 0 ? `${m}m ${s}s` : `${m}m`;
    }
    return `${seconds}s`;
  }

  onMount(async () => {
    try {
      entities = await api.entities();
    } finally {
      isLoading = false;
    }
  });

  $effect(() => {
    document.title = monster
      ? `${monster.display_name} — New Aeven Bestiary`
      : 'Bestiary — New Aeven World Statistics';
  });
</script>

{#if isLoading}
  <div class="space-y-4">
    <div class="h-36 animate-pulse rounded-xl border border-[var(--panel-border)] bg-[var(--panel-soft)]"></div>
    <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
      {#each Array(6) as _, i}
        <div class="h-24 animate-pulse rounded-xl border border-[var(--panel-border)] bg-[var(--panel-soft)]"></div>
      {/each}
    </div>
  </div>
{:else if !monster}
  <div class="space-y-4 rounded-xl border border-[var(--panel-border)] bg-[var(--panel)] p-6">
    <h1 class="text-2xl text-[var(--text)]">Monster not found</h1>
    <p class="text-[var(--text-soft)]">No data exists for "{monsterId}".</p>
    <a href="/world/bestiary" class="pixel-btn inline-flex rounded-md bg-[var(--panel-soft)] px-3 py-2 text-sm text-[var(--text)]">Back to Bestiary</a>
  </div>
{:else}
  <div class="space-y-5">
    <section class="pixel-box relative overflow-hidden rounded-xl bg-[radial-gradient(circle_at_25%_10%,rgba(180,60,60,0.18),transparent_50%),radial-gradient(circle_at_90%_0%,rgba(212,168,68,0.14),transparent_48%),var(--panel)] p-6 md:p-7">
      <p class="flex items-center gap-2 text-xs tracking-[0.22em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">
        <Skull size={14} class="text-[var(--ember)]" />
        Bestiary Entry
      </p>
      <h1 class="mt-2 text-4xl text-[var(--text)]">{monster.display_name}</h1>
      {#if monster.description}
        <p class="mt-2 text-sm text-[var(--text-soft)]">{monster.description}</p>
      {/if}
      <div class="mt-3 flex flex-wrap gap-2 text-sm">
        <span class="rounded-full border border-[var(--panel-border)] bg-[var(--panel-soft)] px-3 py-1 font-mono text-[var(--text-soft)]">Level {monster.level}</span>
        <span class="rounded-full px-3 py-1 font-medium {monster.hostile ? 'border border-[var(--ember)]/30 bg-[var(--ember)]/10 text-[var(--ember)]' : 'border border-[var(--moss)]/30 bg-[var(--moss)]/10 text-[var(--moss-light)]'}">
          {monster.hostile ? 'Aggressive' : 'Passive'}
        </span>
      </div>
      <div class="mt-5">
        <a href="/world/bestiary" class="pixel-btn inline-flex items-center gap-1.5 rounded-md bg-[var(--panel-soft)] px-4 py-2 text-sm font-bold text-[var(--text-soft)] hover:text-[var(--text)]">
          <ArrowLeft size={14} />
          Bestiary
        </a>
      </div>
    </section>

    <section class="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
      {#each [
        { label: 'Hitpoints', value: monster.max_hp.toString() },
        { label: 'Damage', value: monster.damage.toString() },
        { label: 'Attack Bonus', value: signed(monster.attack_bonus) },
        { label: 'Defence Bonus', value: signed(monster.defence_bonus) },
        { label: 'Attack Range', value: `${monster.attack_range} tile${monster.attack_range !== 1 ? 's' : ''}` },
        { label: 'Aggro Range', value: `${monster.aggro_range} tiles` },
        { label: 'Respawn', value: formatRespawn(monster.respawn_time_ms) },
        { label: 'Base XP', value: monster.exp_base.toString() },
      ] as stat}
        <article class="pixel-box rounded-xl bg-[var(--panel)] p-4">
          <p class="text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">{stat.label}</p>
          <p class="mt-2 text-2xl font-bold text-[var(--text)]">{stat.value}</p>
        </article>
      {/each}
    </section>

    <section class="pixel-box space-y-3 rounded-xl bg-[var(--panel)] p-4 md:p-5">
      <p class="flex items-center gap-2 text-[11px] tracking-[0.2em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">
        <TrendingUp size={13} class="text-[var(--gold)]" />
        Level Scaling
      </p>
      <p class="text-xs text-[var(--text-soft)]">HP scales +10% per level, damage +15% per level, XP and gold multiply by level.</p>
      <div class="overflow-x-auto rounded-xl border border-[var(--panel-border)]">
        <table class="w-full min-w-[500px]">
          <thead>
            <tr class="bg-[var(--panel-soft)]">
              <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Level</th>
              <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">HP</th>
              <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Damage</th>
              <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">XP</th>
              <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Gold</th>
            </tr>
          </thead>
          <tbody>
            {#each scalingRows as row}
              <tr class="border-t border-[var(--panel-border)] {row.level === monster.level ? 'bg-[var(--gold)]/8' : 'hover:bg-[var(--panel-soft)]/70'}">
                <td class="px-4 py-2 font-mono text-sm text-[var(--text-soft)]">
                  {row.level}
                  {#if row.level === monster.level}
                    <span class="ml-2 text-[10px] text-[var(--gold)]">BASE</span>
                  {/if}
                </td>
                <td class="px-4 py-2 font-mono text-sm text-[var(--text)]">{row.hp}</td>
                <td class="px-4 py-2 font-mono text-sm text-[var(--text)]">{row.damage}</td>
                <td class="px-4 py-2 font-mono text-sm text-[var(--text)]">{row.exp}</td>
                <td class="px-4 py-2 font-mono text-sm text-[var(--text)]">{row.goldMin === row.goldMax ? row.goldMin : `${row.goldMin}–${row.goldMax}`}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>

    {#if monster.loot.length > 0}
      <section class="pixel-box space-y-3 rounded-xl bg-[var(--panel)] p-4 md:p-5">
        <p class="flex items-center gap-2 text-[11px] tracking-[0.2em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">
          <Droplets size={13} class="text-[var(--water)]" />
          Drop Table
        </p>
        <div class="overflow-x-auto rounded-xl border border-[var(--panel-border)]">
          <table class="w-full">
            <thead>
              <tr class="bg-[var(--panel-soft)]">
                <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Item</th>
                <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Drop Chance</th>
                <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Quantity</th>
              </tr>
            </thead>
            <tbody>
              {#each monster.loot as drop}
                <tr class="border-t border-[var(--panel-border)] hover:bg-[var(--panel-soft)]/70">
                  <td class="px-4 py-2 text-sm font-medium text-[var(--text)]">{formatItemName(drop.item_id)}</td>
                  <td class="px-4 py-2 font-mono text-sm {drop.drop_chance >= 1 ? 'text-[var(--moss-light)]' : 'text-[var(--text)]'}">{formatChance(drop.drop_chance)}</td>
                  <td class="px-4 py-2 font-mono text-sm text-[var(--text-soft)]">{drop.quantity_min === drop.quantity_max ? drop.quantity_min : `${drop.quantity_min}–${drop.quantity_max}`}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </section>
    {/if}

    {#each monster.loot_tables as table}
      {@const totalWeight = table.entries.reduce((sum, e) => sum + e.weight, 0)}
      <section class="pixel-box space-y-3 rounded-xl bg-[var(--panel)] p-4 md:p-5">
        <p class="flex items-center gap-2 text-[11px] tracking-[0.2em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">
          <Dice5 size={13} class="text-[var(--gold)]" />
          {formatItemName(table.name)} Roll
          {#if table.chance < 1}
            <span class="ml-1 text-[var(--text-soft)]">({formatChance(table.chance)} activation)</span>
          {/if}
        </p>
        <div class="overflow-x-auto rounded-xl border border-[var(--panel-border)]">
          <table class="w-full">
            <thead>
              <tr class="bg-[var(--panel-soft)]">
                <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Item</th>
                <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Chance</th>
                <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Quantity</th>
              </tr>
            </thead>
            <tbody>
              {#each table.entries.filter((e) => e.item_id !== 'nothing') as entry}
                {@const pct = totalWeight > 0 ? entry.weight / totalWeight : 0}
                <tr class="border-t border-[var(--panel-border)] hover:bg-[var(--panel-soft)]/70">
                  <td class="px-4 py-2 text-sm font-medium text-[var(--text)]">{formatItemName(entry.item_id)}</td>
                  <td class="px-4 py-2 font-mono text-sm {pct >= 1 ? 'text-[var(--moss-light)]' : 'text-[var(--text)]'}">{formatChance(pct)}</td>
                  <td class="px-4 py-2 font-mono text-sm text-[var(--text-soft)]">{entry.quantity_min === entry.quantity_max ? entry.quantity_min : `${entry.quantity_min}–${entry.quantity_max}`}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </section>
    {/each}

    {#if monster.quest_ids.length > 0}
      <section class="pixel-box space-y-3 rounded-xl bg-[var(--panel)] p-4 md:p-5">
        <p class="flex items-center gap-2 text-[11px] tracking-[0.2em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">
          <ScrollText size={13} class="text-[var(--gold)]" />
          Related Quests
        </p>
        <div class="flex flex-wrap gap-2">
          {#each monster.quest_ids as qid}
            <span class="rounded-full border border-[var(--panel-border)] bg-[var(--panel-soft)] px-3 py-1 text-sm text-[var(--text-soft)]">{formatItemName(qid)}</span>
          {/each}
        </div>
      </section>
    {/if}
  </div>
{/if}
