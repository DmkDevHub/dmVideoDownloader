import { expect, test } from 'vitest';
import fixtures from '../../shared/protocol-fixtures.json';
import { validateWire } from '../../shared/protocol';
test('validates the same wire fixtures as the Rust boundary', () => {
  for (const [wire, error] of fixtures) expect(validateWire(wire!), wire!).toBe(error);
});
