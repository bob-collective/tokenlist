import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import camelCase from 'lodash/fp/camelCase';
import kebabCase from 'lodash/fp/kebabCase';
import { mapByName } from './utils';

test('mapByName preserves normalized keys and last-write collision behavior', () => {
  const first = { name: 'USD Coin', value: 1 };
  const last = { name: 'usd-coin', value: 2 };
  assert.deepEqual(mapByName([first, last]), { 'usd-coin': last });
  assert.deepEqual(mapByName([]), {});
});

test('mapByName matches the previous fp normalization for registry names and edge cases', () => {
  const registry = JSON.parse(
    readFileSync(new URL('./tokenlist.json', import.meta.url), 'utf8'),
  ) as { tokens: { name: string }[] };
  assert.ok(registry.tokens.length > 0);
  const names = [
    ...registry.tokens.map((token) => token.name),
    '',
    'FOOBar',
    'déjà vu',
    'Bitcoin BTC.b',
    "Bob's Token",
    'BTC+',
    '你好 世界',
    '👋 Token 123',
  ];
  for (const name of names) {
    const item = { name };
    assert.deepEqual(mapByName([item]), { [kebabCase(camelCase(name))]: item });
  }
});
