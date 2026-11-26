import { TOKEN_DIR, TOKENLIST_BASE_URL, TOKENLIST_MIRROR_URL } from './paths';
import type { TokenId } from './token-ids';

// `URL` resolves the leading `./` of TOKEN_DIR against the host, so no
// `node:path` is needed and the helpers run unchanged in a browser bundle.
function resolveLogoURI(
  baseURL: string,
  tokenId: TokenId,
  logoext: 'svg' | 'webp',
): string {
  return new URL(`${TOKEN_DIR}/${tokenId}/logo.${logoext}`, baseURL).toString();
}

export function getLogoURI(tokenId: TokenId, logoext: 'svg' | 'webp'): string {
  return resolveLogoURI(TOKENLIST_BASE_URL, tokenId, logoext);
}

export function getMirrorLogoURI(
  tokenId: TokenId,
  logoext: 'svg' | 'webp',
): string {
  return resolveLogoURI(TOKENLIST_MIRROR_URL, tokenId, logoext);
}
