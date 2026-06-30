import { base58check } from '@scure/base';
import camelCase from 'lodash/fp/camelCase';
import kebabCase from 'lodash/fp/kebabCase';
import { bytesToHex, getAddress, hexToBytes, isAddress, sha256 } from 'viem';
import type { KebabCase } from './types';

// Tron addresses are base58check-encoded: a 21-byte payload of the 0x41 mainnet
// prefix followed by the same 20-byte account hash an EVM address uses. Decoding
// it here with a base58check codec (checksum = double SHA-256, via viem's hash)
// replaces the multi-megabyte `tronweb` SDK, which consumers were otherwise
// forced to bundle just for this address conversion.
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

    // Drop the 0x41 Tron prefix → the 20-byte EVM-style address.
    if (decoded) return bytesToHex(decoded.slice(1));
  }

  return getAddress(address);
}

export function toTronAddress(address: string): string {
  if (isAddress(address)) {
    return tronBase58.encode(
      hexToBytes(`0x${TRON_ADDRESS_PREFIX.toString(16)}${address.slice(2)}`),
    );
  }

  return address;
}
