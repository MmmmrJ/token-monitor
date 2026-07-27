#!/usr/bin/env node
/**
 * Build latest.json for Tauri updater from collected release assets.
 * Tauri v2 (`createUpdaterArtifacts: true`) reuses the signed NSIS Setup EXE
 * on Windows; `.nsis.zip` only appears with `"v1Compatible"`.
 */
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

export function buildLatestJson({
  files,
  version,
  tag,
  repo,
  readSignature,
  now = new Date()
}) {
  const find = (rx) => files.find((name) => rx.test(name));
  const appTar = find(/\.app\.tar\.gz$/);
  // Prefer v1Compatible zip when present; otherwise Tauri v2 signed setup.exe.
  const windowsArtifact = find(/\.nsis\.zip$/) || find(/-setup\.exe$/i);
  if (!appTar) throw new Error('missing macOS updater archive (.app.tar.gz)');
  if (!windowsArtifact) {
    throw new Error('missing Windows updater artifact (.nsis.zip or *-setup.exe)');
  }

  const appSig = `${appTar}.sig`;
  const windowsSigName = `${windowsArtifact}.sig`;
  if (!files.includes(appSig)) throw new Error(`missing signature for ${appTar}`);
  if (!files.includes(windowsSigName)) {
    throw new Error(`missing signature for ${windowsArtifact}`);
  }

  const base = `https://github.com/${repo}/releases/download/${tag}`;
  const darwinSig = readSignature(appSig);
  const windowsSig = readSignature(windowsSigName);
  if (!darwinSig) throw new Error(`empty signature for ${appTar}`);
  if (!windowsSig) throw new Error(`empty signature for ${windowsArtifact}`);

  const darwin = { url: `${base}/${appTar}`, signature: darwinSig };
  const platforms = {
    'darwin-aarch64': darwin,
    'darwin-x86_64': { ...darwin },
    'windows-x86_64': { url: `${base}/${windowsArtifact}`, signature: windowsSig }
  };

  for (const [key, value] of Object.entries(platforms)) {
    if (!value.signature) throw new Error(`${key} signature missing`);
  }

  return {
    version,
    notes: `Token Monitor ${tag}`,
    pub_date: now.toISOString().replace(/\.\d{3}Z$/, 'Z'),
    platforms
  };
}

export function main(root = 'release-assets') {
  const files = fs.readdirSync(root);
  const version = process.env.GITHUB_REF_NAME.replace(/^v/, '');
  const tag = process.env.GITHUB_REF_NAME;
  const repo = process.env.GITHUB_REPOSITORY;
  const latest = buildLatestJson({
    files,
    version,
    tag,
    repo,
    readSignature: (name) => fs.readFileSync(path.join(root, name), 'utf8').trim()
  });
  fs.writeFileSync(path.join(root, 'latest.json'), `${JSON.stringify(latest, null, 2)}\n`);
  console.log(JSON.stringify(latest, null, 2));
}

const isDirectRun = process.argv[1]
  && import.meta.url === pathToFileURL(process.argv[1]).href;

if (isDirectRun && process.env.BUILD_LATEST_JSON === '1') {
  main(process.argv[2] || 'release-assets');
}
