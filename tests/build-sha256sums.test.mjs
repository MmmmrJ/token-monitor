import test from 'node:test';
import assert from 'node:assert/strict';
import { buildSha256Sums } from '../scripts/build-sha256sums.mjs';

test('SHA256SUMS covers installers updater assets and latest.json', () => {
  const files = [
    'Token Monitor_1.3.0_universal.dmg',
    'Token Monitor_1.3.0_x64-setup.exe',
    'Token Monitor_1.3.0_x64-setup.exe.sig',
    'Token Monitor.app.tar.gz',
    'latest.json',
    'SHA256SUMS.txt'
  ];
  const blobs = Object.fromEntries(files.map((name) => [name, Buffer.from(name)]));
  const body = buildSha256Sums(files, (name) => blobs[name]);
  assert.match(body, /Token Monitor_1\.3\.0_universal\.dmg/);
  assert.match(body, /Token Monitor_1\.3\.0_x64-setup\.exe/);
  assert.match(body, /Token Monitor\.app\.tar\.gz/);
  assert.match(body, /latest\.json/);
  assert.doesNotMatch(body, /SHA256SUMS\.txt/);
  assert.equal(body.trim().split('\n').length, 5);
});
