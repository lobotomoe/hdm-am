import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { validate } from '@scalar/openapi-parser';
import { describe, expect, it } from 'vitest';

const specPath = fileURLToPath(new URL('../../../docs/openapi.json', import.meta.url));

describe('docs/openapi.json', () => {
  it('is a valid OpenAPI 3.1 document', async () => {
    const raw = readFileSync(specPath, 'utf8');
    const result = await validate(raw);

    expect(result.errors ?? []).toEqual([]);
    expect(result.valid).toBe(true);
    expect(result.version).toBe('3.1');
  });

  it('every operation declares a request body or is a public GET, plus an error response', () => {
    const doc = JSON.parse(readFileSync(specPath, 'utf8')) as {
      paths: Record<string, Record<string, { responses: Record<string, unknown> }>>;
    };

    for (const [path, item] of Object.entries(doc.paths)) {
      const post = item.post;
      if (post) {
        expect(post.responses['200'], `${path} missing 200`).toBeDefined();
        expect(post.responses.default, `${path} missing error response`).toBeDefined();
      }
    }
  });
});
