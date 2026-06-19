/** SPL token mint shown as the public contract address on the homepage. */
export const SOLSTEAD_TOKEN_MINT =
	import.meta.env.VITE_SOLSTEAD_MINT_ADDRESS ??
	'Ez1JTZYnPJicwV4rhtfuhUPnyQHMaBQAXNksPbZ9pump';

export const SOLSTEAD_TOKEN_SYMBOL = import.meta.env.VITE_SOLSTEAD_TOKEN_SYMBOL ?? 'SOLST';

export const SOLSTEAD_CHAIN_CLUSTER = import.meta.env.VITE_SOLSTEAD_CHAIN_CLUSTER ?? 'mainnet';

export function shortenAddress(address: string, head = 6, tail = 6): string {
	if (address.length <= head + tail + 3) return address;
	return `${address.slice(0, head)}…${address.slice(-tail)}`;
}
