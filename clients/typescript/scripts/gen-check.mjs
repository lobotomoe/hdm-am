// Verify src/generated/openapi.ts is in sync with ../../docs/openapi.json.
// Regenerates to a temp file and compares byte-for-byte; exits non-zero on drift.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const committed = fileURLToPath(new URL('../src/generated/openapi.ts', import.meta.url));
const spec = fileURLToPath(new URL('../../../docs/openapi.json', import.meta.url));

const dir = mkdtempSync(join(tmpdir(), 'hdm-gen-'));
const fresh = join(dir, 'openapi.ts');
try {
  // Keep these flags identical to the `gen` script in package.json.
  execFileSync('openapi-typescript', [spec, '--default-non-nullable', 'false', '-o', fresh], {
    stdio: 'inherit',
  });
  const a = readFileSync(committed, 'utf8');
  const b = readFileSync(fresh, 'utf8');
  if (a !== b) {
    console.error(
      'drift: src/generated/openapi.ts is OUT OF DATE — run `pnpm --filter @hdm-am/client gen`',
    );
    process.exit(1);
  }
  console.log('generated types up to date');
} finally {
  rmSync(dir, { recursive: true, force: true });
}
