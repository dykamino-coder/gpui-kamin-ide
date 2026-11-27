// This standalone CommonJS process also evaluates callbacks in browser pages.
// Use JavaScript lint here without attaching it to the application's TS project.
import js from "@eslint/js"

export default [
  js.configs.recommended,
  {
    files: ["*.cjs"],
    languageOptions: {
      sourceType: "commonjs",
      globals: { Buffer: "readonly", console: "readonly", process: "readonly", setTimeout: "readonly" },
    },
  },
]
