// One-command release prep: bump the workspace version, regenerate every version-derived artifact,
// sync the lockfile, and self-verify — so a release can never ship stale artifacts (as 0.5.1 did,
// where docs/openapi.json was left at 0.5.0 because the regen was a manual step that got skipped).
//
//   node scripts/release.mjs 0.5.2
//
// It does NOT commit, tag, push, or publish — those stay human gates. On success it prints the
// remaining steps. Requires a clean working tree so the resulting diff is exactly the release.
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const run = (cmd, args) => {
  console.log(`\n> ${cmd} ${args.join(' ')}`);
  execFileSync(cmd, args, { cwd: repoRoot, stdio: 'inherit' });
};

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error('usage: node scripts/release.mjs <semver>   e.g. 0.5.2');
  process.exit(1);
}

// Refuse to run on a dirty tree: the whole point is that the post-run diff IS the release.
const dirty = execFileSync('git', ['status', '--porcelain'], { cwd: repoRoot, encoding: 'utf8' }).trim();
if (dirty) {
  console.error('working tree is not clean — commit or stash first:\n' + dirty);
  process.exit(1);
}

// Bump the single source of truth: [workspace.package] version. Every crate inherits it via
// `version.workspace = true`, so this one edit moves the whole workspace.
const cargoPath = join(repoRoot, 'Cargo.toml');
const cargo = readFileSync(cargoPath, 'utf8');
const bumped = cargo.replace(
  /(\[workspace\.package\][\s\S]*?\nversion = ")[^"]+(")/,
  `$1${version}$2`,
);
if (bumped === cargo) {
  console.error('could not find [workspace.package] version in Cargo.toml');
  process.exit(1);
}
writeFileSync(cargoPath, bumped);
console.log(`Set workspace version to ${version}`);

// Regenerate all derived artifacts for the new version, then sync the lockfile.
run('node', ['scripts/regen.mjs']);
run('cargo', ['check', '--workspace', '--quiet']);

// Self-verify: the same drift gates CI runs. If any is stale here, the release would fail CI.
run('cargo', ['run', '-q', '-p', 'hdm-am', '--example', 'dump-schema', '--features', 'schema', '--', '--check']);
run('cargo', ['run', '-q', '-p', 'hdm-am-bridge', '--example', 'dump-openapi', '--features', 'schema', '--', '--check']);
run('pnpm', ['-r', '--if-present', 'run', 'gen:check']);

console.log(`\nRelease ${version} prepared and verified. Next:`);
console.log(`  1. Add a CHANGELOG.md entry under [${version}]`);
console.log(`  2. git add -A && git commit -m "chore(release): hdm-am ${version}"`);
console.log(`  3. git tag v${version} && git push --follow-tags`);
console.log(`  4. cargo publish -p hdm-am   (after CI is green)`);
