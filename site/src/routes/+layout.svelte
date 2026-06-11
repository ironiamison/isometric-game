<script lang="ts">
  import '../app.css';
  import { captureUtms } from '$lib/utm';
  import { onMount } from 'svelte';

  let { children } = $props();

  onMount(() => {
    captureUtms();
  });
</script>

{@render children()}
