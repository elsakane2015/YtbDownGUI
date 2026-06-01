#!/usr/bin/env node

import { createPublicKey } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { cwd, env, exit, platform } from "node:process";

const EXPECTED_PRODUCT_NAME = "YtbDownGUI";
const EXPECTED_IDENTIFIER = "com.litotime.ytbdowngui";
const PRODUCTION_LICENSE_URL = "https://license.ytbdown.litotime.com";
const PRO_MIN_VERSION = "1.0.1";

const args = new Set(process.argv.slice(2));
const allowDirty = args.has("--allow-dirty");
const skipSidecars = args.has("--skip-sidecars");

const root = cwd();
const errors = [];
const warnings = [];
const checks = [];

function compareSemver(left, right) {
  const leftParts = left.split(".").map(Number);
  const rightParts = right.split(".").map(Number);
  for (let index = 0; index < 3; index += 1) {
    const diff = (leftParts[index] || 0) - (rightParts[index] || 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

function readJson(path) {
  return JSON.parse(readFileSync(join(root, path), "utf8"));
}

function readText(path) {
  return readFileSync(join(root, path), "utf8");
}

function addError(code, message) {
  errors.push({ code, message });
}

function addWarning(code, message) {
  warnings.push({ code, message });
}

function addCheck(message) {
  checks.push(message);
}

function normalizePem(value) {
  return value.replace(/\\n/g, "\n").trim();
}

function currentGitBranch() {
  try {
    return execFileSync("git", ["rev-parse", "--abbrev-ref", "HEAD"], {
      cwd: root,
      encoding: "utf8",
    }).trim();
  } catch {
    return "";
  }
}

function releaseChannel() {
  const configured = (env.YTBDOWN_RELEASE_CHANNEL || "").trim().toLowerCase();
  if (configured) return configured;
  return currentGitBranch() === "pro-dev" ? "pro" : "free";
}

function validateGitClean() {
  if (allowDirty) {
    addWarning("git_dirty_skipped", "Git worktree check skipped by --allow-dirty.");
    return;
  }
  try {
    const status = execFileSync("git", ["status", "--short"], {
      cwd: root,
      encoding: "utf8",
    }).trim();
    if (status) {
      addError("git_dirty", "Release builds require a clean git worktree.");
    } else {
      addCheck("Git worktree is clean");
    }
  } catch (error) {
    addWarning("git_status_unavailable", `Could not check git status: ${error.message}`);
  }
}

function validateVersions() {
  const packageJson = readJson("package.json");
  const tauriConfig = readJson("src-tauri/tauri.conf.json");
  const cargoToml = readText("src-tauri/Cargo.toml");
  const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  const buildNumber = readText(".buildnumber").trim();

  if (tauriConfig.productName !== EXPECTED_PRODUCT_NAME) {
    addError("product_name", `Tauri productName must remain ${EXPECTED_PRODUCT_NAME}.`);
  }
  if (tauriConfig.identifier !== EXPECTED_IDENTIFIER) {
    addError(
      "identifier",
      `Tauri identifier must remain ${EXPECTED_IDENTIFIER}; Pro and free users share one app id.`,
    );
  }

  const versions = [packageJson.version, tauriConfig.version, cargoVersion].filter(Boolean);
  if (new Set(versions).size !== 1) {
    addError(
      "version_mismatch",
      `package.json, tauri.conf.json, and Cargo.toml versions must match. Saw: ${versions.join(", ")}.`,
    );
  } else {
    addCheck(`App version is consistent at ${versions[0]}`);
  }

  const channel = releaseChannel();
  if (!["free", "pro"].includes(channel)) {
    addError("release_channel", "YTBDOWN_RELEASE_CHANNEL must be free or pro.");
  } else {
    addCheck(`Release channel is ${channel}`);
  }
  if (channel === "pro" && compareSemver(packageJson.version, PRO_MIN_VERSION) < 0) {
    addError("pro_version", `Pro releases must start at v${PRO_MIN_VERSION}.`);
  }

  if (!/^\d{3,}$/.test(buildNumber)) {
    addError("build_number", ".buildnumber must be a zero-padded numeric build number.");
  } else {
    addCheck(`Build number file is valid (${buildNumber})`);
  }
}

function validateLicenseConfig() {
  const configuredUrl = (env.YTBDOWN_LICENSE_SERVER_URL || PRODUCTION_LICENSE_URL).trim();
  if (configuredUrl !== PRODUCTION_LICENSE_URL) {
    addError(
      "license_server_url",
      `Release builds must point to ${PRODUCTION_LICENSE_URL}; got ${configuredUrl}.`,
    );
  } else {
    addCheck("License Server URL points to production");
  }

  const publicKey = normalizePem(env.YTBDOWN_LICENSE_PUBLIC_KEY || "");
  if (!publicKey) {
    addError("license_public_key", "YTBDOWN_LICENSE_PUBLIC_KEY is required for release builds.");
    return;
  }
  if (publicKey.includes("...") || publicKey.toLowerCase().includes("example")) {
    addError("license_public_key", "YTBDOWN_LICENSE_PUBLIC_KEY still looks like a placeholder.");
    return;
  }

  try {
    const key = createPublicKey(publicKey);
    if (key.asymmetricKeyType !== "ed25519") {
      addError("license_public_key", "YTBDOWN_LICENSE_PUBLIC_KEY must be an Ed25519 SPKI public key.");
      return;
    }
    addCheck("License public key is a valid Ed25519 SPKI public key");
  } catch (error) {
    addError("license_public_key", `YTBDOWN_LICENSE_PUBLIC_KEY is invalid: ${error.message}`);
  }
}

function validateSidecars() {
  if (skipSidecars) {
    addWarning("sidecars_skipped", "Sidecar binary check skipped by --skip-sidecars.");
    return;
  }

  const required =
    platform === "win32"
      ? [
          "src-tauri/binaries/yt-dlp-x86_64-pc-windows-msvc.exe",
          "src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe",
        ]
      : [
          "src-tauri/binaries/yt-dlp-universal-apple-darwin",
          "src-tauri/binaries/ffmpeg-universal-apple-darwin",
          "src-tauri/binaries/yt-dlp-aarch64-apple-darwin",
          "src-tauri/binaries/ffmpeg-aarch64-apple-darwin",
          "src-tauri/binaries/yt-dlp-x86_64-apple-darwin",
          "src-tauri/binaries/ffmpeg-x86_64-apple-darwin",
        ];

  const missing = required.filter((path) => !existsSync(join(root, path)));
  if (missing.length > 0) {
    addError("sidecars_missing", `Missing release sidecar binaries: ${missing.join(", ")}.`);
  } else {
    addCheck(`Release sidecar binaries are present for ${platform}`);
  }
}

validateGitClean();
validateVersions();
validateLicenseConfig();
validateSidecars();

console.log("YtbDownGUI release preflight\n");
if (checks.length > 0) {
  console.log("Checks:");
  for (const check of checks) console.log(`  [ok] ${check}`);
  console.log("");
}
if (warnings.length > 0) {
  console.log("Warnings:");
  for (const warning of warnings) console.log(`  [warn] ${warning.code}: ${warning.message}`);
  console.log("");
}
if (errors.length > 0) {
  console.log("Errors:");
  for (const error of errors) console.log(`  [error] ${error.code}: ${error.message}`);
  console.log("");
}

console.log(errors.length === 0 ? "Result: ok" : "Result: failed");
if (errors.length > 0) exit(1);
