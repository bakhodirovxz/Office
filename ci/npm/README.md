# uz-translit

Uzbek Latin <-> Cyrillic transliteration as WebAssembly. It is the same Rust engine the office suite uses
(rules: `docs/phase2/rules.md`), so the web app and the editors give identical results. ESM only; browsers and Node >= 18.
Not published to any registry yet: install the tarball, `npm install ./uz-translit-0.1.0.tgz`.

## API

```ts
toCyrillic(text: string, opts?: TranslitOptions): string
toLatin(text: string, opts?: TranslitOptions): string
convert(text: string, to: "cyrl" | "latn", opts?: TranslitOptions): string
convertRuns(runs: Run[], to: "cyrl" | "latn", opts?: TranslitOptions): string[]   // one paragraph, one result per run
detect(text: string): "latn" | "cyrl" | undefined
version(): string        // package version
rulesVersion(): string   // store it with converted text
```

`TranslitOptions` (all optional):

| option          | values                                | default           |                                                                       |
|-----------------|---------------------------------------|-------------------|-----------------------------------------------------------------------|
| `latinAlphabet` | `"1995"`, `"2026"`, `"auto"`          | `"1995"`          | alphabet written by `toLatin`, read by `toCyrillic` (`auto`: per paragraph) |
| `apostrophe`    | `"official"`, `"ascii"`               | `"official"`      | Latin output: o&#x2BB; g&#x2BB; (U+02BB/U+02BC) or plain `'`           |
| `foreignGuard`  | boolean                               | `true`            | `toCyrillic` keeps words with c, w or diacritics unchanged            |
| `romanNumerals` | boolean                               | `true`            | `toCyrillic` keeps Roman numerals                                     |
| `langPolicy`    | `"convert-ru-en"`, `"skip-ru-en"`     | `"convert-ru-en"` | for runs given as `{text, lang}`: which language tags are converted   |

A run is a string, or `{ text, lang? }` when it has a BCP-47 tag. Runs with a skipped language are returned unchanged and act as
boundaries. Concatenating `convertRuns(split(t))` always equals `convert(t)`, so rich-text editors can convert run by run
without breaking digraphs (sh, ch, o', g') across formatting boundaries. Invalid option values throw an `Error`.

## Browser (Vite, webpack 5, plain ESM)

```js
import init, { toCyrillic, toLatin, detect } from "uz-translit";

await init();                       // fetches uz_translit_bg.wasm next to the module; call once
toCyrillic("O'zbekiston");          // "Ўзбекистон"
toLatin("Ўзбекистон");              // "Oʻzbekiston"
toLatin("Шаҳар", { latinAlphabet: "2026" });
detect("salom");                    // "latn"
```

With Vite the `.wasm` URL is resolved by `new URL(..., import.meta.url)` inside the package; no plugin is needed.
If your bundler cannot do that, pass it yourself: `import wasmUrl from "uz-translit/uz_translit_bg.wasm?url"; await init({ module_or_path: wasmUrl });`.

## Rich text (runs)

```js
const runs = ["O", "'zbek", { text: "tili", lang: "uz-Latn" }, { text: " the", lang: "en" }];
convertRuns(runs, "cyrl", { langPolicy: "skip-ru-en" });
```

## Node >= 18

```js
import { readFileSync } from "node:fs";
import { initSync, toCyrillic } from "uz-translit";

initSync({ module: readFileSync(new URL(import.meta.resolve("uz-translit/uz_translit_bg.wasm"))) });
toCyrillic("salom");
```

Not for React Native / Hermes (no WebAssembly).

License: MIT OR Apache-2.0.
