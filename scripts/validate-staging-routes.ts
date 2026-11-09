// Checks that every token the staging Gateway API routes exists in data/ on
// the same chain and address, and opens one GitHub issue listing any that
// are missing. See "Staging routes validation" in AGENTS.md.

import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { glob } from 'glob';
import { GATEWAY_CHAIN_ALIASES, TOKEN_DIR } from '../config';
import type { TokenData } from '../types';

const DEFAULT_ROUTES_URL =
  'https://gateway-api-staging.gobob.xyz/v4/get-routes';
const ISSUE_LABEL = 'staging-routes-monitor';
const UNKNOWN_CHAIN = 'unknown chain';
const FETCH_TIMEOUT_MS = 30_000;
const MAX_RETRIES = 2;
const RETRY_BASE_DELAY_MS = 2_000;
const ROUTE_FIELDS = ['srcChain', 'dstChain', 'srcToken', 'dstToken'] as const;

type Route = Record<(typeof ROUTE_FIELDS)[number], string>;
type Side = 'src' | 'dst';

type RouteToken = {
  apiChain: string;
  address: string;
  normalizedAddress: string;
  seenAs: Set<Side>;
};

type MissingToken = RouteToken & { tokenlistChain: string };

type GitHubIssue = {
  number: number;
  body?: string | null;
  pull_request?: unknown;
};

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const errorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

const snippet = (text: string) => {
  const flat = text.replace(/\s+/g, ' ').trim();
  return flat.length > 200 ? `${flat.slice(0, 200)}…` : flat;
};

// EVM hex addresses are compared case-insensitively. Anything else (Tron and
// Solana base58) is case-sensitive, so it is compared exactly.
function normalizeAddress(address: string): string {
  return /^0x[0-9a-fA-F]{40}$/.test(address) ? address.toLowerCase() : address;
}

async function fetchText(url: string): Promise<string> {
  let lastError = '';

  for (let attempt = 0; attempt <= MAX_RETRIES; attempt++) {
    if (attempt > 0) {
      const delay = RETRY_BASE_DELAY_MS * 2 ** (attempt - 1);
      console.warn(`${lastError}; retrying in ${delay / 1000}s`);
      await sleep(delay);
    }

    let status: number;
    let body: string;
    try {
      const response = await fetch(url, {
        headers: { accept: 'application/json' },
        signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
      });
      status = response.status;
      body = await response.text();
    } catch (error) {
      lastError = `GET ${url} failed: ${errorMessage(error)}`;
      continue;
    }

    if (status >= 200 && status < 300) return body;

    lastError = `GET ${url} returned HTTP ${status}: ${snippet(body)}`;
    if (status < 500) throw new Error(lastError);
  }

  throw new Error(`${lastError} (gave up after ${MAX_RETRIES + 1} attempts)`);
}

// A broken or empty response must never be read as "nothing missing", so
// anything other than a non-empty array of complete routes is an error.
function parseRoutes(body: string, source: string): Route[] {
  let data: unknown;
  try {
    data = JSON.parse(body);
  } catch {
    throw new Error(`${source} is not valid JSON: ${snippet(body)}`);
  }

  if (!Array.isArray(data)) {
    throw new Error(
      `${source} is not a JSON array of routes: ${snippet(body)}`,
    );
  }
  if (data.length === 0) {
    throw new Error(
      `${source} returned an empty route list; refusing to report nothing missing`,
    );
  }

  return data.map((entry: unknown, index) => {
    if (typeof entry !== 'object' || entry === null) {
      throw new Error(
        `${source}: route ${index} is not an object: ${JSON.stringify(entry)}`,
      );
    }

    const record = entry as Record<string, unknown>;
    const route = {} as Route;
    for (const field of ROUTE_FIELDS) {
      const value = record[field];
      if (typeof value !== 'string' || value === '') {
        throw new Error(
          `${source}: route ${index} has a missing or non-string "${field}": ${JSON.stringify(entry)}`,
        );
      }
      route[field] = value;
    }
    return route;
  });
}

function collectRouteTokens(routes: Route[]): RouteToken[] {
  const tokens = new Map<string, RouteToken>();

  const add = (apiChain: string, address: string, side: Side) => {
    const normalizedAddress = normalizeAddress(address);
    const key = `${apiChain}:${normalizedAddress}`;
    const token = tokens.get(key) ?? {
      apiChain,
      address,
      normalizedAddress,
      seenAs: new Set<Side>(),
    };
    token.seenAs.add(side);
    tokens.set(key, token);
  };

  for (const route of routes) {
    add(route.srcChain, route.srcToken, 'src');
    add(route.dstChain, route.dstToken, 'dst');
  }

  return [...tokens.values()];
}

// data.json chain key → normalized addresses of every token on that chain.
function loadTokenlistAddresses(): Map<string, Set<string>> {
  const addresses = new Map<string, Set<string>>();

  for (const file of glob.sync(path.join(TOKEN_DIR, '*', 'data.json'))) {
    const data: TokenData = JSON.parse(fs.readFileSync(file, 'utf8'));
    for (const [chain, token] of Object.entries(data.tokens)) {
      const chainAddresses = addresses.get(chain) ?? new Set<string>();
      chainAddresses.add(normalizeAddress(token.address));
      addresses.set(chain, chainAddresses);
    }
  }

  if (addresses.size === 0) {
    throw new Error(`No token data found under ${TOKEN_DIR}`);
  }
  return addresses;
}

function findMissingTokens(
  tokens: RouteToken[],
  tokenlist: Map<string, Set<string>>,
): MissingToken[] {
  const missing: MissingToken[] = [];

  for (const token of tokens) {
    const chainKey = GATEWAY_CHAIN_ALIASES[token.apiChain] ?? token.apiChain;
    const chainAddresses = tokenlist.get(chainKey);

    if (!chainAddresses) {
      missing.push({ ...token, tokenlistChain: UNKNOWN_CHAIN });
    } else if (!chainAddresses.has(token.normalizedAddress)) {
      missing.push({ ...token, tokenlistChain: chainKey });
    }
  }

  const compare = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
  return missing.sort(
    (a, b) =>
      compare(a.apiChain, b.apiChain) ||
      compare(a.normalizedAddress, b.normalizedAddress),
  );
}

// Values come from the Gateway API, so keep them inside inline code: no
// table breaks, links or @-mentions in the issue.
function code(value: string): string {
  return `\`${value.replace(/[`\r\n]/g, '').replace(/\|/g, '\\|')}\``;
}

function seenAsLabel(seenAs: Set<Side>): string {
  return seenAs.size === 2 ? 'src+dst' : [...seenAs][0];
}

function workflowRunUrl(): string | undefined {
  const { GITHUB_SERVER_URL, GITHUB_REPOSITORY, GITHUB_RUN_ID } = process.env;
  if (!GITHUB_SERVER_URL || !GITHUB_REPOSITORY || !GITHUB_RUN_ID) return;
  return `${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}`;
}

function issueMarker(missing: MissingToken[]): string {
  const lines = missing
    .map((token) => `${token.apiChain}:${token.normalizedAddress}`)
    .sort();
  const hash = createHash('sha256').update(lines.join('\n')).digest('hex');
  return `<!-- ${ISSUE_LABEL}:${hash} -->`;
}

function buildIssue(missing: MissingToken[], source: string) {
  const title = `Staging routes: ${missing.length} token(s) missing from tokenlist`;
  const runUrl = workflowRunUrl();
  const marker = issueMarker(missing);

  const body = [
    'Checked every token used by the staging Gateway routes against `data/tokens/*/data.json`. Chain and address must both match.',
    '',
    `- Routes: ${source}`,
    `- Checked at: ${new Date().toISOString()}`,
    ...(runUrl ? [`- Workflow run: ${runUrl}`] : []),
    '',
    '| Chain (API) | Tokenlist chain | Address | Seen as |',
    '| --- | --- | --- | --- |',
    ...missing.map(
      (token) =>
        `| ${code(token.apiChain)} | ${token.tokenlistChain === UNKNOWN_CHAIN ? UNKNOWN_CHAIN : code(token.tokenlistChain)} | ${code(token.address)} | ${seenAsLabel(token.seenAs)} |`,
    ),
    '',
    `To fix: add the token as described in AGENTS.md "Adding a new token". For an \`${UNKNOWN_CHAIN}\`, add an alias to \`GATEWAY_CHAIN_ALIASES\` in \`config.ts\` or add the chain.`,
    '',
    marker,
  ].join('\n');

  return { title, body, marker };
}

function gitHubClient() {
  const token = process.env.GITHUB_TOKEN;
  const repo = process.env.GITHUB_REPOSITORY;
  if (!token || !repo) {
    throw new Error(
      'GITHUB_TOKEN and GITHUB_REPOSITORY must be set to open an issue (use --dry-run to print it instead)',
    );
  }
  const apiUrl = process.env.GITHUB_API_URL || 'https://api.github.com';

  return async function request<T>(
    method: string,
    endpoint: string,
    body?: unknown,
  ): Promise<{ status: number; data: T }> {
    const response = await fetch(`${apiUrl}/repos/${repo}${endpoint}`, {
      method,
      headers: {
        accept: 'application/vnd.github+json',
        authorization: `Bearer ${token}`,
        'content-type': 'application/json',
        'user-agent': 'bob-tokenlist-validate-staging-routes',
        'x-github-api-version': '2022-11-28',
      },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
    });
    const text = await response.text();
    let data: unknown = {};
    try {
      data = text ? JSON.parse(text) : {};
    } catch {
      data = { message: snippet(text) };
    }
    return { status: response.status, data: data as T };
  };
}

type GitHubRequest = ReturnType<typeof gitHubClient>;

async function ensureLabel(request: GitHubRequest): Promise<void> {
  const { status, data } = await request<{ errors?: { code?: string }[] }>(
    'POST',
    '/labels',
    {
      name: ISSUE_LABEL,
      color: 'd93f0b',
      description: 'Tokens routed by the staging Gateway but missing here',
    },
  );
  const alreadyExists =
    status === 422 &&
    data.errors?.some((error) => error.code === 'already_exists');
  if (status !== 201 && !alreadyExists) {
    throw new Error(
      `Creating label ${ISSUE_LABEL} failed: HTTP ${status} ${snippet(JSON.stringify(data))}`,
    );
  }
}

async function findOpenIssueWithMarker(
  request: GitHubRequest,
  marker: string,
): Promise<number | undefined> {
  for (let page = 1; ; page++) {
    const { status, data } = await request<GitHubIssue[]>(
      'GET',
      `/issues?state=open&labels=${ISSUE_LABEL}&per_page=100&page=${page}`,
    );
    if (status !== 200) {
      throw new Error(
        `Listing ${ISSUE_LABEL} issues failed: HTTP ${status} ${snippet(JSON.stringify(data))}`,
      );
    }

    const match = data.find(
      (issue) => !issue.pull_request && issue.body?.includes(marker),
    );
    if (match) return match.number;
    if (data.length < 100) return;
  }
}

async function reportMissing(
  issue: ReturnType<typeof buildIssue>,
): Promise<void> {
  const request = gitHubClient();

  const existing = await findOpenIssueWithMarker(request, issue.marker);
  if (existing !== undefined) {
    console.log(`Missing tokens already reported in #${existing}`);
    return;
  }

  await ensureLabel(request);
  const { status, data } = await request<{
    number?: number;
    html_url?: string;
  }>('POST', '/issues', {
    title: issue.title,
    body: issue.body,
    labels: [ISSUE_LABEL],
  });
  if (status !== 201) {
    throw new Error(
      `Creating issue failed: HTTP ${status} ${snippet(JSON.stringify(data))}`,
    );
  }
  console.log(`Opened #${data.number}: ${data.html_url}`);
}

async function main() {
  const { values } = parseArgs({
    options: {
      'routes-file': { type: 'string' },
      'dry-run': { type: 'boolean', default: false },
    },
  });
  const dryRun =
    values['dry-run'] ||
    ['1', 'true'].includes(process.env.DRY_RUN?.toLowerCase() ?? '');

  const routesFile = values['routes-file'];
  const routesUrl = process.env.ROUTES_URL?.trim() || DEFAULT_ROUTES_URL;
  const source = routesFile ? `file \`${routesFile}\`` : routesUrl;
  const body = routesFile
    ? fs.readFileSync(routesFile, 'utf8')
    : await fetchText(routesUrl);

  const routes = parseRoutes(body, source);
  const tokens = collectRouteTokens(routes);
  const missing = findMissingTokens(tokens, loadTokenlistAddresses());

  console.log(
    `Checked ${routes.length} routes, ${tokens.length} unique tokens, ${missing.length} missing`,
  );
  if (missing.length === 0) return;

  const issue = buildIssue(missing, source);
  if (dryRun) {
    console.log(
      `\n--- dry run: issue not created ---\n${issue.title}\n\n${issue.body}`,
    );
    return;
  }
  await reportMissing(issue);
}

main().catch((error) => {
  console.error(`validate-staging-routes: ${errorMessage(error)}`);
  process.exitCode = 1;
});
