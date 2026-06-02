import camelCase from 'lodash/fp/camelCase';
import kebabCase from 'lodash/fp/kebabCase';
import { TronWeb } from 'tronweb';
import { getAddress, isAddress } from 'viem';
import type { KebabCase } from './types';

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
  if (address.startsWith('T') && TronWeb.isAddress(address)) return address;

  return getAddress(address);
}

export function toEvmAddress(address: string): string {
  if (address.startsWith('T') && TronWeb.isAddress(address)) {
    const tronHex = TronWeb.address.toHex(address);

    return `0x${tronHex.slice(2)}`;
  }

  return getAddress(address);
}

export function toTronAddress(address: string): string {
  if (isAddress(address)) {
    const tronHex = `41${address.slice(2)}`;

    return TronWeb.address.fromHex(tronHex);
  }

  return address;
}
