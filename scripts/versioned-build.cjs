const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));

function pad(value) {
  return String(value).padStart(2, "0");
}

function timestamp() {
  const now = new Date();
  return [
    now.getFullYear(),
    pad(now.getMonth() + 1),
    pad(now.getDate()),
    "-",
    pad(now.getHours()),
    pad(now.getMinutes()),
  ].join("");
}

const version = pkg.version;
const stamp = timestamp();
const exeName = `${pkg.name}_${version}_${stamp}.exe`;
const source = path.join(root, "src-tauri", "target", "release", "api-switch.exe");
const outDir = path.join(root, "release");
const target = path.join(outDir, exeName);

if (!fs.existsSync(source)) {
  console.error(`Build output not found: ${source}`);
  console.error("Run `pnpm tauri build` before renaming the local build.");
  process.exit(1);
}

fs.mkdirSync(outDir, { recursive: true });
fs.copyFileSync(source, target);
console.log(`Created ${path.relative(root, target)}`);
