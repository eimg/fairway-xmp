import assert from 'node:assert/strict';
import test from 'node:test';
import { isPgError } from '../src/postgresql/explorer-types.ts';
test('PostgreSQL job errors retain commit uncertainty and safe SQLSTATE separately', () => {
  const error = { code: 'cancelled', message: 'Cancelled', sqlState: null, position: null, outcome: 'unknown' };
  assert.equal(isPgError(error), true);
  assert.equal(isPgError({ ...error, outcome: 'rolledBackGuaranteed' }), false);
  assert.equal(isPgError({ ...error, sqlState: 57014 }), false);
  assert.equal(isPgError({ ...error, code: 'mysqlError' }), false);
});
