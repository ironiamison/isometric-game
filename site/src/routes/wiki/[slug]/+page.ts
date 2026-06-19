import { error } from '@sveltejs/kit';
import { ALL_ARTICLES, ARTICLE_BY_SLUG } from '$lib/wiki';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ params }) => {
  const article = ARTICLE_BY_SLUG.get(params.slug);
  if (!article) {
    error(404, 'Article not found');
  }
  return { article };
};

export const prerender = true;

export function entries() {
  return ALL_ARTICLES.map((a) => ({ slug: a.slug }));
}
