<script lang="ts">
  import { page } from '$app/stores';
  import {
    Gamepad2,
    Gem,
    LayoutGrid,
    Menu,
    Skull,
    Trophy,
    Users,
    X,
  } from '@lucide/svelte';

  let { children } = $props();
  let mobileOpen = $state(false);

  const navItems = [
    { href: '/world', label: 'Overview', icon: LayoutGrid, exact: true },
    { href: '/world/players', label: 'Players', icon: Users, exact: false },
    { href: '/world/leaderboards', label: 'Leaderboards', icon: Trophy, exact: false },
    { href: '/world/items', label: 'Items', icon: Gem, exact: false },
    { href: '/world/bestiary', label: 'Bestiary', icon: Skull, exact: false },
  ];

  function isActive(href: string, exact: boolean) {
    const path = $page.url.pathname;
    if (exact) return path === href || path === `${href}/`;
    return path === href || path.startsWith(`${href}/`);
  }
</script>

<div class="relative min-h-screen overflow-x-clip bg-[var(--bg)]">
  <div
    aria-hidden="true"
    class="pointer-events-none fixed inset-0 bg-[radial-gradient(900px_480px_at_12%_-10%,rgba(90,64,30,0.22),transparent_62%),radial-gradient(800px_440px_at_96%_0%,rgba(212,168,68,0.1),transparent_58%),radial-gradient(1100px_700px_at_50%_100%,rgba(42,30,20,0.28),transparent_65%)]"
  ></div>

  <header class="sticky top-0 z-40 border-b border-[var(--panel-border)]/50 bg-[var(--bg)]/80 px-6 backdrop-blur-md md:px-10">
    <div class="mx-auto flex max-w-6xl items-center justify-between py-3">
      <a href="/world">
        <span
          class="text-base font-bold tracking-[0.22em] text-[var(--gold)]"
          style="font-family: var(--font-display)"
        >
          NEW AEVEN
        </span>
      </a>

      <nav class="hidden items-center gap-1 md:flex">
        {#each navItems as item}
          <a
            href={item.href}
            class="relative flex items-center gap-1.5 px-3 py-1.5 text-[13px] font-semibold tracking-wide transition-colors duration-150 {isActive(item.href, item.exact)
              ? 'text-[var(--gold)]'
              : 'text-[var(--text-soft)] hover:text-[var(--text)]'}"
          >
            <item.icon size={13} strokeWidth={2} class="shrink-0 -translate-y-px" />
            {item.label}
            {#if isActive(item.href, item.exact)}
              <span class="absolute bottom-0 left-3 right-3 h-px bg-[var(--gold)]"></span>
            {/if}
          </a>
        {/each}
      </nav>

      <a
        href="/#play"
        class="hidden items-center gap-1.5 text-[10px] tracking-[0.12em] text-[var(--muted)] uppercase transition-colors hover:text-[var(--text-soft)] md:inline-flex"
        style="font-family: var(--font-display)"
      >
        <Gamepad2 size={13} />
        Play game
      </a>

      <button
        onclick={() => (mobileOpen = !mobileOpen)}
        class="flex h-8 w-8 items-center justify-center text-[var(--text-soft)] md:hidden"
        aria-label="Toggle menu"
      >
        {#if mobileOpen}
          <X size={18} />
        {:else}
          <Menu size={18} />
        {/if}
      </button>
    </div>

    {#if mobileOpen}
      <nav class="border-t border-[var(--panel-border)]/30 bg-[var(--bg)]/95 px-6 py-3 backdrop-blur-md md:hidden">
        <div class="flex flex-col gap-1">
          {#each navItems as item}
            <a
              href={item.href}
              onclick={() => (mobileOpen = false)}
              class="flex items-center gap-2 rounded-md px-3 py-2 text-sm font-semibold transition-colors {isActive(item.href, item.exact)
                ? 'text-[var(--gold)]'
                : 'text-[var(--text-soft)] active:text-[var(--text)]'}"
            >
              <item.icon size={15} strokeWidth={2} />
              {item.label}
            </a>
          {/each}
          <div class="mt-2 border-t border-[var(--panel-border)]/30 pt-2">
            <a
              href="/#play"
              class="flex items-center gap-2 px-3 py-2 text-xs tracking-[0.1em] text-[var(--muted)] uppercase"
              style="font-family: var(--font-display)"
            >
              <Gamepad2 size={13} />
              Play game
            </a>
          </div>
        </div>
      </nav>
    {/if}
  </header>

  <main class="relative z-10 px-6 py-8 md:px-10 md:py-10">
    <div class="mx-auto max-w-6xl">
      {@render children()}
    </div>
  </main>
</div>
