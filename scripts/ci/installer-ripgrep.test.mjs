import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { crc32, deflateRawSync, zstdCompressSync } from "node:zlib";
import test from "node:test";

// Маленький ZIP с central directory: exe сжат deflate, лицензия stored.
function archive(entries) {
  const locals = [], central = [];
  let offset = 0;
  for (const [name, data, method] of entries) {
    const filename = Buffer.from(name);
    const packed = method === 8 ? deflateRawSync(data) : data;
    const local = Buffer.alloc(30), record = Buffer.alloc(46);
    local.writeUInt32LE(0x04034b50);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(method, 8);
    local.writeUInt32LE(crc32(data), 14);
    local.writeUInt32LE(packed.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(filename.length, 26);
    record.writeUInt32LE(0x02014b50);
    record.writeUInt16LE(20, 6);
    record.writeUInt16LE(method, 10);
    record.writeUInt32LE(crc32(data), 16);
    record.writeUInt32LE(packed.length, 20);
    record.writeUInt32LE(data.length, 24);
    record.writeUInt16LE(filename.length, 28);
    record.writeUInt32LE(offset, 42);
    locals.push(local, filename, packed);
    central.push(record, filename);
    offset += local.length + filename.length + packed.length;
  }
  const directory = Buffer.concat(central), end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, directory, end]);
}

test("installer extracts ripgrep when the tar on PATH rejects drive-letter paths", () => {
  const root = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
  // Для red-прогона подменяется только исходник установщика старой версией.
  const source = readFileSync(process.env.KAMIN_INSTALLER_TEST_SOURCE ?? join(root, "scripts/build_installer.mjs"), "utf8");
  const directory = /dir: "([^"]+)"/.exec(source)[1];
  const exe = Buffer.from("MZ synthetic ripgrep executable\0\xff", "latin1");
  const license = Buffer.from("Synthetic MIT license\r\n");
  const zip = archive([[`${directory}/rg.exe`, exe, 8], [`${directory}/LICENSE-MIT`, license, 0]]);
  const hash = createHash("sha256").update(zip).digest("hex");
  const fixture = mkdtempSync(join(tmpdir(), "kamin-installer-rg-"));
  const put = (name, data) => {
    const target = join(fixture, name);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, data);
  };
  try {
    put("Cargo.toml", 'version = "0.0.0"\n');
    put("target/release/kaminide-gpui.exe", "synthetic shell");
    put("target/release/kamin-jobrun.exe", "synthetic job runner");
    put("target/ripgrep/" + directory + ".zip", zip);
    put("payload/runtime.tar.zst", zstdCompressSync(Buffer.alloc(1024)));
    put("package.json", "{}");
    put("node_modules/tar/package.json", '{"main":"index.cjs"}');
    put("node_modules/tar/index.cjs", "exports.extract = async () => {};\n");
    put("scripts/build-runtime-payload.mjs", "// Синтетический payload уже актуален.\n");
    // Выполняется реальный сборщик; закреплённый digest заменён хешем фикстуры.
    put("scripts/build_installer.mjs", source.replace(/sha256: "[a-f0-9]{64}"/, `sha256: "${hash}"`));
    put("reject-tar.cjs", `
const cp = require('node:child_process');
const original = cp.spawnSync;
cp.spawnSync = (command, ...args) => command === 'tar'
  ? (process.stderr.write('GNU tar: Cannot connect to D: resolve failed\\n'), { status: 1 })
  : original(command, ...args);
require('node:module').syncBuiltinESMExports();
`);
    const result = spawnSync(process.execPath, ["--require", join(fixture, "reject-tar.cjs"), join(fixture, "scripts/build_installer.mjs")], {
      cwd: fixture, encoding: "utf8", env: { ...process.env, KAMIN_DEV_REPO: fixture }, timeout: 30000,
    });
    assert.equal(result.status, 0, result.stdout + result.stderr);
    assert.deepEqual(readFileSync(join(fixture, "dist-installer/rg.exe")), exe);
    assert.deepEqual(readFileSync(join(fixture, "dist-installer/ripgrep-LICENSE-MIT.txt")), license);
  } finally {
    rmSync(fixture, { recursive: true, force: true });
  }
});
