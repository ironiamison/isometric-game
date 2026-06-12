<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type OnlinePlayer } from '$lib/api';
  import { Users } from '@lucide/svelte';

  type SortKey = keyof OnlinePlayer;
  type SortDir = 'asc' | 'desc';

  let data: OnlinePlayer[] | undefined = $state();
  let isLoading = $state(true);
  let sortKey: SortKey = $state('combat_level');
  let sortDir: SortDir = $state('desc');

  const columns: { key: SortKey; label: string }[] = [
    { key: 'name', label: 'Name' },
    { key: 'combat_level', label: 'Combat Lv' },
    { key: 'hitpoints_level', label: 'Hitpoints' },
    { key: 'attack_level', label: 'Attack' },
    { key: 'strength_level', label: 'Strength' },
    { key: 'defence_level', label: 'Defence' },
    { key: 'ranged_level', label: 'Ranged' },
    { key: 'total_level', label: 'Total Lv' },
  ];

  let sorted = $derived.by(() => {
    if (!data) return [];
    return [...data].sort((a, b) => {
      const av = a[sortKey];
      const bv = b[sortKey];
      if (av < bv) return sortDir === 'asc' ? -1 : 1;
      if (av > bv) return sortDir === 'asc' ? 1 : -1;
      return 0;
    });
  });

  function toggleSort(key: SortKey) {
    if (sortKey === key) {
      sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    } else {
      sortKey = key;
      sortDir = 'desc';
    }
  }

  async function load() {
    try {
      data = await api.online();
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    document.title = 'Online Players — New Aeven World Statistics';
    load();
    const id = setInterval(load, 15_000);
    return () => clearInterval(id);
  });
</script>

<svelte:head>
  <title>Online Players — New Aeven World Statistics</title>
</svelte:head>

<div class="space-y-6">
  <div class="flex items-center gap-3">
    <Users size={22} class="text-[var(--gold)]" />
    <h1 class="text-2xl font-bold text-[var(--text)]">Online Players</h1>
    {#if data}
      <span class="rounded-full bg-[var(--gold)] px-3 py-0.5 text-sm font-bold text-[#1a1210]">{data.length}</span>
    {/if}
  </div>

  <div class="pixel-box overflow-x-auto rounded-lg bg-[var(--panel)]">
    <table class="w-full">
      <thead>
        <tr class="bg-[var(--panel-soft)]">
          {#each columns as col}
            <th
              onclick={() => toggleSort(col.key)}
              class="cursor-pointer px-4 py-3 text-left text-xs tracking-wider text-[var(--muted)] uppercase select-none transition-colors hover:text-[var(--text)]"
            >
              {col.label}
              {#if sortKey === col.key}
                <span class="ml-1">{sortDir === 'asc' ? '▲' : '▼'}</span>
              {/if}
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#if isLoading}
          {#each Array(6) as _, i}
            <tr class="border-b border-[var(--panel-border)]">
              {#each columns as col}
                <td class="px-4 py-3">
                  <div class="h-4 w-16 animate-pulse rounded bg-[var(--panel-soft)]"></div>
                </td>
              {/each}
            </tr>
          {/each}
        {:else if sorted.length === 0}
          <tr>
            <td colspan={columns.length} class="px-4 py-12 text-center text-[var(--muted)]">Nobody's online right now</td>
          </tr>
        {:else}
          {#each sorted as player}
            <tr class="border-b border-[var(--panel-border)] transition-colors hover:bg-[var(--panel-soft)]">
              <td class="px-4 py-3">
                <a href="/world/player/{encodeURIComponent(player.name)}" class="text-[var(--text)] hover:text-[var(--gold)]">{player.name}</a>
              </td>
              <td class="px-4 py-3 font-mono text-[var(--text)]">{player.combat_level}</td>
              <td class="px-4 py-3 font-mono text-[var(--text)]">{player.hitpoints_level}</td>
              <td class="px-4 py-3 font-mono text-[var(--text)]">{player.attack_level}</td>
              <td class="px-4 py-3 font-mono text-[var(--text)]">{player.strength_level}</td>
              <td class="px-4 py-3 font-mono text-[var(--text)]">{player.defence_level}</td>
              <td class="px-4 py-3 font-mono text-[var(--text)]">{player.ranged_level}</td>
              <td class="px-4 py-3 font-mono text-[var(--text)]">{player.total_level}</td>
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>
</div>
