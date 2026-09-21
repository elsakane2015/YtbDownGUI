import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

function argument(name, required = true) {
  const index = process.argv.indexOf(`--${name}`);
  const value = index >= 0 ? process.argv[index + 1] : undefined;
  if (required && !value) throw new Error(`missing --${name}`);
  return value;
}

const file = resolve(argument("file"));
const tag = argument("tag");
const version = argument("version");
const asset = resolve(argument("asset"));
const signatureFile = resolve(argument("signature"));
const targets = argument("targets")
  .split(",")
  .map((target) => target.trim())
  .filter(Boolean);
const repository = argument("repository", false) || "elsakane2015/YtbDownGUI";
const notes = argument("notes", false);

if (!existsSync(asset)) throw new Error(`asset not found: ${asset}`);
if (!existsSync(signatureFile)) throw new Error(`signature not found: ${signatureFile}`);

const manifest = existsSync(file)
  ? JSON.parse(readFileSync(file, "utf8"))
  : { version, notes: "", pub_date: new Date().toISOString(), platforms: {} };

manifest.version = version;
manifest.pub_date = new Date().toISOString();
manifest.platforms ||= {};
if (notes) manifest.notes = notes;

const encodedTag = encodeURIComponent(tag);
const encodedAsset = encodeURIComponent(basename(asset));
const url = `https://github.com/${repository}/releases/download/${encodedTag}/${encodedAsset}`;
const signature = readFileSync(signatureFile, "utf8").trim();
for (const target of targets) {
  manifest.platforms[target] = { url, signature };
}

writeFileSync(file, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Updated ${file}: ${targets.join(", ")} -> ${basename(asset)}`);
