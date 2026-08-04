// Strict ESLint flat config (ESLint 9+). The bar: bad TypeScript should
// not even reach `npm run dev`. Run with `npm run lint`. CI gates merge
// on `lint` + `typecheck` passing.
//
// Why strict-type-checked: catches a class of bugs that the TS compiler
// (in `noImplicitAny + strict`) misses — unsafe Promise discards, unsafe
// enum compares, `as` casts that erase soundness. Worth the slower run.
import js from "@eslint/js"
import { createTypeScriptImportResolver } from "eslint-import-resolver-typescript"
import importX from "eslint-plugin-import-x"
import tseslint from "typescript-eslint"

export default tseslint.config(
  {
    ignores: [
      "node_modules/**",
      "dist/**",
      "dist-tauri/**",
      "dist-host/**",
      "out/**",
      ".vite/**",
      // Rust shell (R2): no TS to lint — Cargo target/ holds generated
      // tauri-codegen `.js` assets that would flood the type-checker.
      "src-tauri/**",
      // Дев-скрипты стенда (probe/installer) — plain JS вне ts-project.
      "scripts/**",
      "target/**",
      "vendor/**",
      "crates/**",
      "dist-installer/**",
      "extensions-test-corpus/**",
      // Local reference clone of microsoft/vscode (gitignored) — not our code,
      // has its own eslint config + deps we don't install.
      "docs/vscode-ref/**",
      "builtin-extensions/**/extension.js",
      // Bridge VSIX build projects (extensions/claude-bridge/{webview,extension}):
      // self-contained Vite/esbuild builds with their own tsconfig. The webview
      // is a 1:1 copy of electron/renderer and the extension vendors
      // electron/main — neither follows kamin-ide's lint rules; both are
      // typechecked by their own `tsc`. Only the built artifacts ship.
      "extensions/**",
      "scripts/download-test-corpus.mjs",
      // The compat-smoke script hand-rolls a `vscode` shim and runs
      // un-typed VSIX code through it; eslint's strict-type-checked
      // config has no realistic chance of typing that surface.
      "scripts/test-corpus-compat.mjs",
      // Perf probes drive the packaged Tauri app over CDP / WMI via
      // Playwright + spawned PowerShell (dynamic `unknown` shapes). The
      // strict-type-checked rule set has no realistic shape for it.
      "scripts/perf-cold-start.mjs",
      "scripts/perf-ram.mjs",
      "scripts/find-tauri-binary.mjs",
      "scripts/launch-cdp.mjs",
      // Node build CLI (fs/tar/zstd, no project types) — same as the
      // other .mjs build scripts: typed linting has nothing to add.
      "scripts/build-runtime-payload.mjs",
      "scripts/tauri-build-dev.mjs",
      // Sibling of tauri-build-dev.mjs above; it was the only build script
      // missing from this list, so the project service failed to resolve it and
      // reported a parse error on every `npm run lint` run.
      "scripts/tauri-build.mjs",
      "tests/e2e/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  ...tseslint.configs.stylisticTypeChecked,
  importX.flatConfigs.recommended,
  importX.flatConfigs.typescript,
  {
    languageOptions: {
      parserOptions: {
        projectService: {
          // Vite/Forge config files live at the repo root, outside the
          // `src/`+`scripts/` tsconfig include glob. Listing them here
          // lets typescript-eslint's project service load them with a
          // sensible default config rather than erroring out.
          allowDefaultProject: [
            "vite.*.config.ts",
            "vitest.config.ts",
            "eslint.config.mjs",
            ".size-limit.cjs",
          ],
          // tseslint hard-fails when the globs above match more than 8
          // files (its perf guard). Adding vite.kamin-host.config.ts made
          // it 9 — all of them two-line configs, so the slowdown warning
          // does not apply in practice.
          maximumDefaultProjectFileMatchCount_THIS_WILL_SLOW_DOWN_LINTING: 12,
        },
        tsconfigRootDir: import.meta.dirname,
      },
    },
    settings: {
      // Wire the standalone TypeScript resolver so import-x can follow
      // `.ts`/`.tsx` paths and the bundler-style `.js`-extension imports
      // we use everywhere. Without it, every import-x rule errors with
      // "invalid interface loaded as resolver".
      "import-x/resolver-next": [
        createTypeScriptImportResolver({
          alwaysTryTypes: true,
          project: "./tsconfig.json",
        }),
      ],
    },
    rules: {
      // ── File size ────────────────────────────────────────────────────────
      // Hard team rule: 250 LOC ceiling per .ts/.tsx. Skip blank lines and
      // comments so docstrings don't push us over — what counts is logic.
      "max-lines": ["error", { max: 250, skipBlankLines: true, skipComments: true }],

      // ── Import hygiene (import-x) ────────────────────────────────────────
      // `no-default-export` is the closest automated proxy for "no barrels":
      // forces named exports so renames stay grep-able. Vite/Forge config
      // files override this below — they require default exports.
      "import-x/no-cycle": ["error", { maxDepth: 6 }],
      "import-x/no-default-export": "error",
      "import-x/no-self-import": "error",
      "import-x/no-useless-path-segments": "error",
      "import-x/order": ["error", { "newlines-between": "never", alphabetize: { order: "asc" } }],

      // ── No magic values. Names + reasons or it doesn't compile. ──────────
      "no-magic-numbers": [
        "error",
        {
          ignore: [-1, 0, 1, 2],
          ignoreArrayIndexes: true,
          ignoreDefaultValues: true,
          ignoreClassFieldInitialValues: true,
          enforceConst: true,
          detectObjects: false,
        },
      ],

      // ── Promise / async safety ───────────────────────────────────────────
      "@typescript-eslint/no-floating-promises": ["error", { ignoreVoid: true }],
      "@typescript-eslint/no-misused-promises": "error",
      "@typescript-eslint/await-thenable": "error",
      "@typescript-eslint/require-await": "error",
      "@typescript-eslint/return-await": ["error", "always"],
      "no-return-await": "off", // superseded by ts version above

      // ── Type strictness ──────────────────────────────────────────────────
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-unsafe-assignment": "error",
      "@typescript-eslint/no-unsafe-member-access": "error",
      "@typescript-eslint/no-unsafe-call": "error",
      "@typescript-eslint/no-unsafe-return": "error",
      "@typescript-eslint/no-unsafe-argument": "error",
      "@typescript-eslint/restrict-template-expressions": ["warn", { allowNumber: true, allowBoolean: true }],

      // ── Unused / dead code ───────────────────────────────────────────────
      "@typescript-eslint/no-unused-vars": [
        "error",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
          caughtErrorsIgnorePattern: "^_",
          destructuredArrayIgnorePattern: "^_",
        },
      ],

      // ── Modern JS only ───────────────────────────────────────────────────
      "no-var": "error",
      "prefer-const": "error",
      "prefer-template": "error",
      "object-shorthand": "error",
      "@typescript-eslint/prefer-nullish-coalescing": "error",
      "@typescript-eslint/prefer-optional-chain": "error",

      // ── Imports / module hygiene ─────────────────────────────────────────
      "@typescript-eslint/consistent-type-imports": ["error", { prefer: "type-imports" }],
      "@typescript-eslint/no-import-type-side-effects": "error",

      // ── Class / function shape ───────────────────────────────────────────
      "@typescript-eslint/no-extraneous-class": ["error", { allowEmpty: false, allowStaticOnly: false }],
      "@typescript-eslint/prefer-readonly": "error",

      // ── Switch exhaustiveness ────────────────────────────────────────────
      "@typescript-eslint/switch-exhaustiveness-check": "error",
      "default-case": "error",
      "no-fallthrough": "error",

      // ── Console policy ───────────────────────────────────────────────────
      // Allow warn/error/info — they're real signals. Forbid raw `console.log`
      // in committed code; use a logger or remove before merge.
      "no-console": ["warn", { allow: ["warn", "error", "info"] }],
    },
  },
  // VS Code shim stub files — empty methods/classes are by-design
  // (Phase A skeleton; full behaviour ships in Phase D). The lint
  // overrides are scoped narrowly so a NEW empty function elsewhere
  // still gets flagged.
  {
    files: ["src/exthost/api/classes-*.ts", "src/exthost/api/ns-*.ts"],
    rules: {
      "@typescript-eslint/no-empty-function": "off",
      "@typescript-eslint/no-extraneous-class": "off",
      "@typescript-eslint/no-unnecessary-condition": "off",
    },
  },
  // Vite requires default exports for its config loaders. Allow exactly
  // here, nowhere else (no-default-export stays a hard error in src/).
  {
    files: ["vite.*.config.ts", "*.config.{ts,js,mjs}"],
    rules: { "import-x/no-default-export": "off" },
  },
  // Built-in extensions are JS, not part of the type-checked program
  {
    files: ["builtin-extensions/**/*.{js,cjs}"],
    rules: { "@typescript-eslint/no-require-imports": "off" },
  },
  // Tests — fixtures use literals freely (assertion values, IDs, sizes).
  // Vitest's `toThrow`/`toThrowError` overlap; we tolerate either since
  // the fix is mechanical and the rule isn't catching real bugs.
  {
    files: ["**/*.test.{ts,tsx}", "**/*.spec.{ts,tsx}"],
    rules: {
      "no-magic-numbers": "off",
      "@typescript-eslint/no-deprecated": "off",
    },
  },
  // ESLint config itself + Vite root configs use namespace-style imports
  // (`import tseslint from "typescript-eslint"` then `tseslint.configs`)
  // because that's the API both libraries publish. The lint rule
  // catching this would force ergonomically worse spelling for no
  // safety win.
  {
    files: ["eslint.config.mjs", "vite.*.config.ts"],
    rules: {
      "import-x/no-named-as-default": "off",
      "import-x/no-named-as-default-member": "off",
      "@typescript-eslint/no-deprecated": "off",
      "@typescript-eslint/no-unsafe-assignment": "off",
      "@typescript-eslint/no-unsafe-member-access": "off",
    },
  },
  // CommonJS config files (.cjs) get the right globals.
  {
    files: ["**/*.cjs"],
    languageOptions: { sourceType: "commonjs", globals: { module: "readonly", require: "readonly", __dirname: "readonly" } },
  },
  // Ambient declaration files: `declare module "*.css"` REQUIRES default
  // exports per the .d.ts spec, so the rule is incorrect there.
  {
    files: ["**/*.d.ts"],
    rules: { "import-x/no-default-export": "off" },
  },
  // Scripts — small CLIs, magic numbers acceptable.
  // `no-named-as-default-member` flags `ts.SyntaxKind` style access on
  // the TypeScript Compiler API namespace import; that pattern is how
  // tsc itself documents the API, so silence it here.
  {
    files: ["scripts/**/*.{ts,mjs}"],
    rules: {
      "no-magic-numbers": "off",
      "no-console": "off",
      "import-x/no-named-as-default-member": "off",
      "import-x/no-default-export": "off",
    },
  },
)
