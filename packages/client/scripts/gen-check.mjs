// Verify generated TypeScript and Zod artifacts are in sync with ../../docs/openapi.json.
// Regenerates to temp files and compares byte-for-byte; exits non-zero on drift.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const generatedOpenapi = fileURLToPath(new URL('../src/generated/openapi.ts', import.meta.url));
const generatedZod = fileURLToPath(new URL('../src/generated/zod.ts', import.meta.url));
const orvalConfig = fileURLToPath(new URL('../orval.config.ts', import.meta.url));
const spec = fileURLToPath(new URL('../../../docs/openapi.json', import.meta.url));

const dir = mkdtempSync(join(tmpdir(), 'hdm-gen-'));
const freshOpenapi = join(dir, 'openapi.ts');
const freshZod = join(dir, 'zod.ts');

function assertSame(label, committedPath, freshPath) {
  const committed = readFileSync(committedPath, 'utf8');
  const fresh = readFileSync(freshPath, 'utf8');
  if (committed !== fresh) {
    console.error(
      `drift: ${label} is OUT OF DATE — run \`pnpm --filter @hdm-am/client gen\``,
    );
    process.exit(1);
  }
}

try {
  execFileSync('openapi-typescript', [spec, '--default-non-nullable', 'false', '-o', freshOpenapi], {
    stdio: 'inherit',
  });
  execFileSync('orval', ['--config', orvalConfig], {
    env: {
      ...process.env,
      HDM_ZOD_TARGET: freshZod,
    },
    stdio: 'inherit',
  });

  assertSame('src/generated/openapi.ts', generatedOpenapi, freshOpenapi);
  assertSame('src/generated/zod.ts', generatedZod, freshZod);
  console.log('generated types and zod schemas up to date');
} finally {
  rmSync(dir, { recursive: true, force: true });
}
