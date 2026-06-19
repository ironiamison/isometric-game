import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

/** Title screen is the site home — not the legacy marketing landing page. */
export const load: PageLoad = () => {
  redirect(307, '/play/index.html');
};

export const prerender = true;
