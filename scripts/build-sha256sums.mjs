#!/usr/bin/env node
/**
 * Write SHA256SUMS.txt for Release assets.
 * Covers installers, updater archives, signatures, and latest.json.
 */
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';

export function buildSha256Sums(files, readFile) {
  const lines = [];
  const names = [...files].sort((a, b) => a.localeCompare(b));
  for (const name of names) {
    if (name === 'SHA256SUMS.txt') continue;
    const hash = crypto.createHash('sha256').update(readFile(name)).digest('hex');
    lines.push(`${hash}  ${name}`);
  }
  return `${lines.join('\n')}${lines.length ? '\n' : ''}`;
}

export function main(root = 'release-assets') {
  const files = fs.readdirSync(root).filter((name) => fs.statSync(path.join(root, name)).isFile());
  const body = buildSha256Sums(files, (name) => fs.readFileSync(path.join(root, name)));
  if (!body.trim()) throw new Error('no release assets to hash');
  fs.writeFileSync(path.join(root, 'SHA256SUMS.txt'), body);
  console.log(body);
}

const isDirectRun = process.argv[1]
  && import.meta.url === pathToFileURL(process.argv[1]).href;

if (isDirectRun && process.env.BUILD_SHA256SUMS === '1') {
  main(process.argv[2] || 'release-assets');
}
