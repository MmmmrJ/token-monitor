#!/usr/bin/env node
/**
 * Build latest.json for Tauri updater from collected release assets.
 * Windows in-app updates must use the signed NSIS archive (.nsis.zip), not Setup.exe.
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
  const nsisZip = find(/\.nsis\.zip$/);
  if (!appTar) throw new Error('missing macOS updater archive (.app.tar.gz)');
  if (!nsisZip) throw new Error('missing Windows NSIS updater archive (.nsis.zip)');

  const appSig = `${appTar}.sig`;
  const nsisSig = `${nsisZip}.sig`;
  if (!files.includes(appSig)) throw new Error(`missing signature for ${appTar}`);
  if (!files.includes(nsisSig)) throw new Error(`missing signature for ${nsisZip}`);

  const base = `https://github.com/${repo}/releases/download/${tag}`;
  const darwinSig = readSignature(appSig);
  const windowsSig = readSignature(nsisSig);
  if (!darwinSig) throw new Error(`empty signature for ${appTar}`);
  if (!windowsSig) throw new Error(`empty signature for ${nsisZip}`);

  const darwin = { url: `${base}/${appTar}`, signature: darwinSig };
  const platforms = {
    'darwin-aarch64': darwin,
    'darwin-x86_64': { ...darwin },
    'windows-x86_64': { url: `${base}/${nsisZip}`, signature: windowsSig }
  };

  for (const [key, value] of Object.entries(platforms)) {
    if (/\.exe$/i.test(value.url)) {
      throw new Error(`${key} updater URL must not point to .exe: ${value.url}`);
    }
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
