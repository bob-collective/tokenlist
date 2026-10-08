#!/usr/bin/env ts-node

import * as fs from 'node:fs';
import * as path from 'node:path';
import {
  arbitrum,
  avalanche,
  base,
  berachain,
  bob,
  bobSepolia,
  bsc,
  hyperEvm,
  mainnet,
  optimism,
  plasma,
  polygon,
  robinhood,
  sei,
  sepolia,
  soneium,
  sonic,
  telos,
  tron,
  unichain,
} from 'viem/chains';
import {
  BITCOIN_CHAIN_ID,
  CHAIN_DIR,
  NON_EVM_CHAIN_ID_BY_NAME,
  SIGNET_CHAIN_ID,
  SOLANA_CHAIN_ID,
  SUPPORTED_CHAIN_MAP,
} from '../config';
import type { Token } from '../types';
import { toEvmAddress } from '../utils';

const API_VERSION = process.env.API_VERSION || 'v4';
const DISCORD_WEBHOOK_URL = process.env.DISCORD_WEBHOOK_URL;

const GATEWAY_ENVIRONMENTS = {
  staging: 'https://gateway-api-staging.gobob.xyz',
  mainnet: 'https://gateway-api-mainnet.gobob.xyz',
} as const;

type Environment = keyof typeof GATEWAY_ENVIRONMENTS;

// Gateway chain name → chain ID, covering every chain in data/chains/chains.json.
// Names for chains the gateway does not route yet are best guesses; an
// unrecognised name surfaces as "unknown chain" in the report.
const GATEWAY_CHAIN_ID_BY_NAME: Partial<Record<string, number>> = {
  arbitrum: arbitrum.id,
  avalanche: avalanche.id,
  base: base.id,
  berachain: berachain.id,
  bitcoin: BITCOIN_CHAIN_ID,
  bob: bob.id,
  'bob-sepolia': bobSepolia.id,
  bsc: bsc.id,
  ethereum: mainnet.id,
  hyperevm: hyperEvm.id,
  optimism: optimism.id,
  plasma: plasma.id,
  polygon: polygon.id,
  robinhood: robinhood.id,
  sei: sei.id,
  sepolia: sepolia.id,
  signet: SIGNET_CHAIN_ID,
  solana: SOLANA_CHAIN_ID,
  soneium: soneium.id,
  sonic: sonic.id,
  telos: telos.id,
  tron: tron.id,
  unichain: unichain.id,
};

// Discord rejects message content longer than 2000 characters.
const DISCORD_MESSAGE_LIMIT = 2000;

interface Route {
  srcChain: string;
  dstChain: string;
  srcToken: string;
  dstToken: string;
}

interface RouteToken {
  chain: string;
  address: string;
}

interface MissingToken extends RouteToken {
  environments: Environment[];
  reason: string;
}

const tokenlistPath = path.join(__dirname, '../tokenlist.json');
const chainsPath = path.join(__dirname, '..', CHAIN_DIR, 'chains.json');

// Fail fast when a chain is added to chains.json without a gateway mapping.
function assertGatewayChainsComplete() {
  const chainKeys = Object.keys(
    JSON.parse(fs.readFileSync(chainsPath, 'utf8')) as Record<string, string>,
  ).filter((key) => key !== '$schema');
  const mappedIds = new Set(Object.values(GATEWAY_CHAIN_ID_BY_NAME));
  const unmapped = chainKeys.filter((key) => {
    const chainId =
      SUPPORTED_CHAIN_MAP[key as keyof typeof SUPPORTED_CHAIN_MAP]?.id ??
      NON_EVM_CHAIN_ID_BY_NAME[key];

    return chainId === undefined || !mappedIds.has(chainId);
  });

  if (unmapped.length > 0) {
    throw new Error(
      `GATEWAY_CHAIN_ID_BY_NAME is missing chains from chains.json: ${unmapped.join(', ')}`,
    );
  }
}

function resolveChainId(chain: string): number | undefined {
  return GATEWAY_CHAIN_ID_BY_NAME[chain];
}

// Tron addresses may appear in base58 or hex form; compare everything as
// lowercase hex. Anything unparseable (e.g. Solana mints) is kept verbatim.
function normalizeAddress(address: string): string {
  try {
    return toEvmAddress(address).toLowerCase();
  } catch {
    return address;
  }
}

function tokenKey(chainId: number, address: string): string {
  return `${chainId}:${normalizeAddress(address)}`;
}

async function fetchRoutes(env: Environment): Promise<Route[]> {
  const url = `${GATEWAY_ENVIRONMENTS[env]}/${API_VERSION}/get-routes`;
  const res = await fetch(url);

  if (!res.ok) {
    throw new Error(`Failed to fetch ${url}: ${res.status} ${res.statusText}`);
  }

  return (await res.json()) as Route[];
}

function collectRouteTokens(routes: Route[]): RouteToken[] {
  const tokens = new Map<string, RouteToken>();

  for (const route of routes) {
    for (const token of [
      { chain: route.srcChain, address: route.srcToken },
      { chain: route.dstChain, address: route.dstToken },
    ]) {
      tokens.set(`${token.chain}:${normalizeAddress(token.address)}`, token);
    }
  }

  return [...tokens.values()];
}

function loadTokenlistKeys(): Set<string> {
  const { tokens } = JSON.parse(fs.readFileSync(tokenlistPath, 'utf8')) as {
    tokens: Token[];
  };

  return new Set(tokens.map((token) => tokenKey(token.chainId, token.address)));
}

function findMissingTokens(
  routeTokensByEnv: Record<Environment, RouteToken[]>,
  tokenlistKeys: Set<string>,
): MissingToken[] {
  const missing = new Map<string, MissingToken>();

  for (const [env, routeTokens] of Object.entries(routeTokensByEnv) as [
    Environment,
    RouteToken[],
  ][]) {
    for (const token of routeTokens) {
      const chainId = resolveChainId(token.chain);
      const isListed =
        chainId !== undefined &&
        tokenlistKeys.has(tokenKey(chainId, token.address));

      if (isListed) continue;

      const key = `${token.chain}:${normalizeAddress(token.address)}`;
      const entry = missing.get(key) ?? {
        ...token,
        environments: [],
        reason:
          chainId === undefined
            ? 'unknown chain'
            : 'token missing from tokenlist',
      };

      entry.environments.push(env);
      missing.set(key, entry);
    }
  }

  return [...missing.values()].sort(
    (a, b) =>
      a.chain.localeCompare(b.chain) || a.address.localeCompare(b.address),
  );
}

function formatReport(missing: MissingToken[]): string[] {
  const header = `⚠️ **Gateway route tokens missing from tokenlist** (API ${API_VERSION}, ${missing.length} token${missing.length === 1 ? '' : 's'})`;
  const lines = missing.map(
    (token) =>
      `- \`${token.chain}\` \`${token.address}\` — ${token.environments.join(', ')}${token.reason === 'unknown chain' ? ' (unknown chain)' : ''}`,
  );

  // Split into messages that each fit within Discord's content limit.
  const messages: string[] = [];
  let current = header;

  for (const line of lines) {
    if (current.length + line.length + 1 > DISCORD_MESSAGE_LIMIT) {
      messages.push(current);
      current = line;
    } else {
      current += `\n${line}`;
    }
  }

  messages.push(current);

  return messages;
}

async function postToDiscord(webhookUrl: string, messages: string[]) {
  for (const content of messages) {
    const res = await fetch(webhookUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content, allowed_mentions: { parse: [] } }),
    });

    if (!res.ok) {
      throw new Error(
        `Failed to post to Discord: ${res.status} ${res.statusText}`,
      );
    }
  }
}

async function report(messages: string[]) {
  console.log(messages.join('\n'));

  if (!DISCORD_WEBHOOK_URL) {
    console.warn('DISCORD_WEBHOOK_URL not set — skipping Discord report');
    return;
  }

  await postToDiscord(DISCORD_WEBHOOK_URL, messages);
  console.log('Report posted to Discord');
}

async function main() {
  assertGatewayChainsComplete();

  const envs = Object.keys(GATEWAY_ENVIRONMENTS) as Environment[];
  const routes = await Promise.all(envs.map(fetchRoutes));
  const routeTokensByEnv = Object.fromEntries(
    envs.map((env, i) => [env, collectRouteTokens(routes[i])]),
  ) as Record<Environment, RouteToken[]>;

  const missing = findMissingTokens(routeTokensByEnv, loadTokenlistKeys());

  if (missing.length === 0) {
    const tokenCount = new Set(
      Object.values(routeTokensByEnv)
        .flat()
        .map((token) => `${token.chain}:${normalizeAddress(token.address)}`),
    ).size;

    await report([
      `✅ **Gateway route tokens all present in tokenlist** (API ${API_VERSION}, ${tokenCount} tokens checked across ${envs.join(', ')})`,
    ]);
    return;
  }

  await report(formatReport(missing));
}

main().catch(async (error) => {
  console.error(error);

  const message = error instanceof Error ? error.message : String(error);
  const { GITHUB_SERVER_URL, GITHUB_REPOSITORY, GITHUB_RUN_ID } = process.env;
  const runUrl =
    GITHUB_SERVER_URL && GITHUB_REPOSITORY && GITHUB_RUN_ID
      ? `\n${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}`
      : '';

  try {
    await report([
      `❌ **Gateway route token check failed** (API ${API_VERSION})\n\`\`\`\n${message.slice(0, 1500)}\n\`\`\`${runUrl}`,
    ]);
  } catch (reportError) {
    console.error(reportError);
  }

  process.exit(1);
});
