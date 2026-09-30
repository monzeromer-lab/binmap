# corpus/web

A purpose-built web target, matching `corpus/stress` on the native side: small
enough to build in seconds, shaped to exercise the things that go wrong.

- `format.ts` — small, eagerly imported, on the critical path.
- `analysis.ts` — the largest module, reachable only through a dynamic import,
  so initial-load bytes and total bytes differ by a lot.
- `constants.ts` — a block of highly repetitive strings that compresses almost
  for free, so that removing it takes away raw bytes without taking away
  transfer bytes. That is `TOOLING-WEB §4.1`'s counterintuitive result, and it
  is here so the disagreement between raw and transfer ranking is reproducible
  rather than theoretical.

Build it two ways:

```bash
npm run build         # one chunk
npm run build:split   # code splitting, so the lazy chunk is separate
```
