// Where token assets live: the directory in this repo, and the two hosts that
// serve it. Kept free of imports so the logo helpers can use them without
// loading `config.ts`.
export const TOKENLIST_BASE_URL =
  'https://raw.githubusercontent.com/bob-collective/tokenlist/refs/heads/main/';
export const TOKENLIST_MIRROR_URL = 'https://static.gobob.xyz/tokenlist/';

export const TOKEN_DIR = './data/tokens';
