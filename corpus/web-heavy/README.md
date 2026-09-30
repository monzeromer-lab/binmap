# corpus/web-heavy

A deliberately badly-configured web target, and the companion to
`corpus/web`'s clean one. Every dependency exists to make one of
`TOOLING-WEB §6`'s findings fire against **real npm packages**, because a rule
that only fires on a metafile written by hand is a rule that has never met
production.

| Import | Rule it triggers |
|---|---|
| `import { debounce } from "lodash"` | whole-library import |
| `core-js/es/*` | polyfill cost |
| `buffer` | Node shim in a browser bundle |

```bash
npm install && npm run build
cargo run -p binmap-eval -- web corpus/web-heavy
```
