/* Headless CSSOM snapshots. Browser binaries and outputs belong in private storage. */
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { pathToFileURL } = require("node:url");

function argumentsFor(argv) {
  const options = {};
  for (let i = 0; i < argv.length; i += 2) {
    if (!argv[i].startsWith("--") || argv[i + 1] === undefined)
      throw new Error("expected --name value arguments");
    const name = argv[i].slice(2);
    if (Object.hasOwn(options, name)) throw new Error(`duplicate --${name}`);
    if (!["input", "output", "width", "height", "browsers-path", "playwright-module", "executable"].includes(name))
      throw new Error(`unknown --${name}`);
    options[name] = argv[i + 1];
  }
  for (const name of ["input", "output", "width", "browsers-path"])
    if (!options[name]) throw new Error(`missing --${name}`);
  for (const name of ["width", "height"]) {
    options[name] = Number(options[name] || 600);
    if (!Number.isInteger(options[name]) || options[name] < 1 || options[name] > 8192)
      throw new Error(`invalid --${name}`);
  }
  if (!path.isAbsolute(options["browsers-path"]))
    throw new Error("--browsers-path must be an absolute private directory");
  return options;
}

async function main() {
  if (process.platform !== "linux") throw new Error("Chromium snapshots require Podman/Linux; host launches are forbidden");
  const options = argumentsFor(process.argv.slice(2));
  process.env.PLAYWRIGHT_BROWSERS_PATH = options["browsers-path"];
  const { chromium } = require(options["playwright-module"] || "playwright");
  const browser = await chromium.launch({ headless: true, args: ["--enable-blink-features=GeometryUtils"],
    ...(options.executable ? { executablePath: options.executable } : {}) });
  try {
    const context = await browser.newContext({ viewport: { width: options.width, height: options.height },
      deviceScaleFactor: 1, reducedMotion: "reduce" });
    const page = await context.newPage();
    const input = /^[a-z]+:\/\//i.test(options.input) ? options.input : pathToFileURL(path.resolve(options.input)).href;
    await page.goto(input, { waitUntil: "load" });
    await page.evaluate(async () => {
      await document.fonts.ready;
      await Promise.all(Array.from(document.images, image => image.decode().catch(() => {})));
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    });
    const elements = await page.evaluate(() => {
      const rect = r => ({ x: r.x, y: r.y, width: r.width, height: r.height });
      if (typeof Element.prototype.getBoxQuads !== "function")
        throw new Error("Chromium must support GeometryUtils.getBoxQuads; install a current Chromium build");
      const boxes = (node, box) => node.getBoxQuads({ box }).map(quad => {
        const r = quad.getBounds();
        return { x: r.x - scrollX, y: r.y - scrollY, width: r.width, height: r.height };
      });
      const result = [];
      const pending = [[document, []]];
      while (pending.length) {
        const [node, indexPath] = pending.pop();
        if (node.nodeType === Node.ELEMENT_NODE) {
          const style = getComputedStyle(node);
          result.push({ path: indexPath, tag: node.localName, id: node.getAttribute("id") || "",
            class: node.getAttribute("class") || "", border_box: rect(node.getBoundingClientRect()),
            client_rects: Array.from(node.getClientRects(), rect), padding_boxes: boxes(node, "padding"), content_boxes: boxes(node, "content"),
            font_size: parseFloat(style.fontSize),
            line_height: style.lineHeight === "normal" ? "normal" : parseFloat(style.lineHeight) });
        }
        for (let i = node.childNodes.length - 1; i >= 0; i--)
          pending.push([node.childNodes[i], [...indexPath, i]]);
      }
      return result;
    });
    fs.writeFileSync(options.output, JSON.stringify({ schema_version: 1,
      viewport: { width: options.width, height: options.height, device_scale_factor: 1 }, elements }, null, 2) + "\n");
  } finally {
    await browser.close();
  }
}

main().catch(error => { console.error(error.message); process.exitCode = 1; });
