import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const requiredFiles = [
  'dist/index.cjs',
  'dist/index.cjs.map',
  'dist/index.d.cts',
  'dist/index.d.ts',
  'dist/index.js',
  'dist/index.js.map',
  'LICENSE-APACHE',
  'LICENSE-MIT',
  'README.md',
  'package.json',
];

const forbiddenPrefixes = ['node_modules/', 'src/', 'test/'];
const forbiddenFiles = new Set(['tsconfig.json', 'tsup.config.ts', 'vitest.config.ts']);
const dependencyFields = ['dependencies', 'peerDependencies', 'optionalDependencies', 'devDependencies'];
const expectedExports = {
  '@hdm-am/client': ['HdmBridgeClient', 'HdmBridgeError', 'HdmTransportError', 'HdmValidationError'],
  '@hdm-am/react': ['HdmProvider', 'useHdmClient', 'useMutation', 'HdmBridgeClient'],
};
const expectedSubpathExports = {
  '@hdm-am/client': {
    './zod': {
      cjs: 'dist/zod.cjs',
      dts: 'dist/zod.d.ts',
      esm: 'dist/zod.js',
      exports: ['createValidatedClient', 'createZodValidation', 'hdmZodResponseValidators'],
    },
  },
};

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function parsePackJson(raw) {
  const trimmed = raw.trim();
  const start = trimmed.indexOf('{');
  const end = trimmed.lastIndexOf('}');
  assert(start !== -1 && end !== -1 && end >= start, `pnpm pack did not emit JSON:\n${raw}`);
  return JSON.parse(trimmed.slice(start, end + 1));
}

function assertNoWorkspaceProtocol(pkg) {
  for (const field of dependencyFields) {
    const deps = pkg[field];
    if (!deps || typeof deps !== 'object') {
      continue;
    }
    for (const [name, range] of Object.entries(deps)) {
      assert(
        typeof range !== 'string' || !range.startsWith('workspace:'),
        `${field}.${name} still uses ${range}`,
      );
    }
  }
}

function packedPackageJson(tarball) {
  const raw = execFileSync('tar', ['-xOf', tarball, 'package/package.json'], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  return JSON.parse(raw);
}

async function assertEntrypointsLoad(pkg) {
  const exportsToCheck = expectedExports[pkg.name] ?? [];
  const esm = await import(pathToFileURL(join(process.cwd(), 'dist/index.js')).href);
  const require = createRequire(join(process.cwd(), 'package.json'));
  const cjs = require('./dist/index.cjs');
  for (const name of exportsToCheck) {
    assert(name in esm, `${pkg.name} ESM entrypoint is missing ${name}`);
    assert(name in cjs, `${pkg.name} CJS entrypoint is missing ${name}`);
  }

  const subpaths = expectedSubpathExports[pkg.name] ?? {};
  for (const [subpath, config] of Object.entries(subpaths)) {
    const subpathEsm = await import(pathToFileURL(join(process.cwd(), config.esm)).href);
    const subpathCjs = require(`./${config.cjs}`);
    for (const name of config.exports) {
      assert(name in subpathEsm, `${pkg.name} ${subpath} ESM entrypoint is missing ${name}`);
      assert(name in subpathCjs, `${pkg.name} ${subpath} CJS entrypoint is missing ${name}`);
    }
  }
}

const cwd = process.cwd();
const tmp = mkdtempSync(join(tmpdir(), 'hdm-pack-'));

try {
  const raw = execFileSync(
    'pnpm',
    ['pack', '--json', '--pack-destination', tmp],
    {
      cwd,
      env: {
        ...process.env,
        npm_config_ignore_scripts: 'true',
      },
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'inherit'],
    },
  );
  const pack = parsePackJson(raw);
  const files = new Set(pack.files.map((file) => file.path));

  for (const file of requiredFiles) {
    assert(files.has(file), `${pack.name} tarball is missing ${file}`);
  }
  const subpaths = expectedSubpathExports[pack.name] ?? {};
  for (const config of Object.values(subpaths)) {
    for (const file of [config.cjs, `${config.cjs}.map`, config.dts, config.esm, `${config.esm}.map`]) {
      assert(files.has(file), `${pack.name} tarball is missing ${file}`);
    }
  }
  for (const file of files) {
    assert(!forbiddenFiles.has(file), `${pack.name} tarball unexpectedly includes ${file}`);
    for (const prefix of forbiddenPrefixes) {
      assert(!file.startsWith(prefix), `${pack.name} tarball unexpectedly includes ${file}`);
    }
  }

  const pkg = packedPackageJson(pack.filename);
  assert(pkg.name === pack.name, `packed package name mismatch: ${pkg.name} !== ${pack.name}`);
  assert(typeof pkg.version === 'string' && pkg.version !== '0.0.0', `${pkg.name} has an invalid version`);
  assert(pkg.private !== true, `${pkg.name} must not be private`);
  assert(pkg.license === 'MIT OR Apache-2.0', `${pkg.name} must keep the dual license`);
  assert(pkg.sideEffects === false, `${pkg.name} must declare sideEffects=false`);
  assert(pkg.publishConfig?.access === 'public', `${pkg.name} must publish with public access`);
  assert(pkg.main === './dist/index.cjs', `${pkg.name} main must point at CJS output`);
  assert(pkg.module === './dist/index.js', `${pkg.name} module must point at ESM output`);
  assert(pkg.types === './dist/index.d.ts', `${pkg.name} types must point at declarations`);
  assert(pkg.exports?.['.']?.types === './dist/index.d.ts', `${pkg.name} export must expose types`);
  assert(pkg.exports?.['.']?.import === './dist/index.js', `${pkg.name} export must expose ESM`);
  assert(pkg.exports?.['.']?.require === './dist/index.cjs', `${pkg.name} export must expose CJS`);
  for (const [subpath, config] of Object.entries(subpaths)) {
    assert(pkg.exports?.[subpath]?.types === `./${config.dts}`, `${pkg.name} ${subpath} must expose types`);
    assert(pkg.exports?.[subpath]?.import === `./${config.esm}`, `${pkg.name} ${subpath} must expose ESM`);
    assert(pkg.exports?.[subpath]?.require === `./${config.cjs}`, `${pkg.name} ${subpath} must expose CJS`);
  }
  assertNoWorkspaceProtocol(pkg);
  await assertEntrypointsLoad(pkg);

  console.log(`${pack.name}@${pack.version} pack ok (${files.size} files)`);
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
