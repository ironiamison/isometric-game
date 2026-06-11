<script lang="ts">
  import { page } from '$app/stores';
</script>

<div class="flex min-h-screen items-center justify-center bg-[var(--bg)] px-6 text-center">
  <div>
    <h1 class="text-4xl text-[var(--gold)]">{$page.status}</h1>
    <p class="mt-2 text-[var(--text-soft)]">{$page.error?.message ?? 'Page not found'}</p>
    <a href="/" class="pixel-btn mt-6 inline-block rounded-md bg-[var(--panel-soft)] px-4 py-2 text-sm text-[var(--text)]">Back home</a>
  </div>
</div>
