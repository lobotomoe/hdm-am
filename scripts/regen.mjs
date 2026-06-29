// Regenerate every artifact derived from the Rust types or the crate version, in one shot:
//   - docs/schema/*.json      (JSON Schema of the wire request/response types)
//   - docs/openapi.json        (the bridge HTTP API, carries the crate version)
//   - packages/*/src/generated (TypeScript types + zod, generated from docs/openapi.json)
//
// Run after changing a wire type OR bumping the version. The matching `--check` variants run in CI
// (`cargo run ... -- --check`, `pnpm -r gen:check`) and fail the build if any of these is stale —
// this script is how you make them green without remembering three separate commands.
//
//   node scripts/regen.mjs
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');

const steps = [
  ['cargo', ['run', '-q', '-p', 'hdm-am', '--example', 'dump-schema', '--features', 'schema']],
  ['cargo', ['run', '-q', '-p', 'hdm-am-bridge', '--example', 'dump-openapi', '--features', 'schema']],
  ['pnpm', ['-r', '--if-present', 'run', 'gen']],
];

for (const [cmd, args] of steps) {
  console.log(`\n> ${cmd} ${args.join(' ')}`);
  execFileSync(cmd, args, { cwd: repoRoot, stdio: 'inherit' });
}

console.log('\nRegenerated schema, openapi, and TypeScript client.');
