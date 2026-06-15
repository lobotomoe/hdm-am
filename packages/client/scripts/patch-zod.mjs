import { readFileSync, writeFileSync } from 'node:fs';

const file = process.argv[2];

if (!file) {
  console.error('usage: node scripts/patch-zod.mjs <generated-zod-file>');
  process.exit(1);
}

const marker = `export const probeResponseResponseCodeMin = 0;
export const probeResponseResponseCodeMax = 65535;
`;

const tupleBounds = `${marker}
export const probeResponseProtocolVersion0ItemMin = 0;
export const probeResponseProtocolVersion0ItemMax = 255;
export const probeResponseProtocolVersion1ItemMin = 0;
export const probeResponseProtocolVersion1ItemMax = 255;
export const probeResponseSoftwareVersion0ItemMin = 0;
export const probeResponseSoftwareVersion0ItemMax = 255;
export const probeResponseSoftwareVersion1ItemMin = 0;
export const probeResponseSoftwareVersion1ItemMax = 255;
export const probeResponseSoftwareVersion2ItemMin = 0;
export const probeResponseSoftwareVersion2ItemMax = 255;
`;

let source = readFileSync(file, 'utf8');

if (!source.includes('export const probeResponseProtocolVersion0ItemMin')) {
  if (!source.includes(marker)) {
    throw new Error('unexpected Orval output: probe response-code marker not found');
  }
  source = source.replace(marker, tupleBounds);
  writeFileSync(file, source);
}
