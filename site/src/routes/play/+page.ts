import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

/** WASM shell lives in static/play/index.html — SvelteKit doesn't map /play/ to it automatically. */
export const load: PageLoad = () => {
  redirect(307, '/play/index.html');
};
