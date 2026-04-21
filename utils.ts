import kebabCase from 'lodash/fp/kebabCase';
import type { KebabCase } from './types';

export function mapByName<T extends { name: string }>(
  arr: T[],
): Record<KebabCase<T['name']>, T> {
  return arr.reduce(
    (acc, object) => {
      acc[kebabCase(object.name) as KebabCase<T['name']>] = object;

      return acc;
    },
    {} as Record<KebabCase<T['name']>, T>,
  );
}
