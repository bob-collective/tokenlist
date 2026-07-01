import { base58check } from '@scure/base';
import camelCase from 'lodash/fp/camelCase';
import kebabCase from 'lodash/fp/kebabCase';
import { bytesToHex, getAddress, hexToBytes, isAddress, sha256 } from 'viem';
import type { KebabCase } from './types';

// A Tron address is base58check over 21 bytes: the 0x41 prefix + a 20-byte EVM address.
const TRON_ADDRESS_PREFIX = 0x41;
const tronBase58 = base58check((data) => sha256(data, 'bytes'));

function decodeTronAddress(address: string): Uint8Array | null {
  try {
    const decoded = tronBase58.decode(address);

    return decoded.length === 21 && decoded[0] === TRON_ADDRESS_PREFIX
      ? decoded
      : null;
  } catch {
    return null;
  }
}

export function mapByName<T extends { name: string }>(
  arr: T[],
): Record<KebabCase<T['name']>, T> {
  return arr.reduce(
    (acc, object) => {
      acc[kebabCase(camelCase(object.name)) as KebabCase<T['name']>] = object;

      return acc;
    },
    {} as Record<KebabCase<T['name']>, T>,
  );
}

export function checksumAddress(address: string): string {
  if (address.startsWith('T') && decodeTronAddress(address)) return address;

  return getAddress(address);
}

export function toEvmAddress(address: string): string {
  if (address.startsWith('T')) {
    const decoded = decodeTronAddress(address);

    if (decoded) return bytesToHex(decoded.slice(1));
  }

  return getAddress(address);
}

export function toTronAddress(address: string): string {
  if (isAddress(address)) {
    const prefix = TRON_ADDRESS_PREFIX.toString(16).padStart(2, '0');

    return tronBase58.encode(hexToBytes(`0x${prefix}${address.slice(2)}`));
  }

  return address;
}
