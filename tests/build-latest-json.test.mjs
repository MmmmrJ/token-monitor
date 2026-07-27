import test from 'node:test';
import assert from 'node:assert/strict';
import { buildLatestJson } from '../scripts/build-latest-json.mjs';

test('latest.json uses signed Tauri v2 setup.exe when nsis.zip is absent', () => {
  const files = [
    'Token Monitor_1.2.0_universal.dmg',
    'Token Monitor_1.2.0_x64_en-US.msi',
    'Token Monitor_1.2.0_x64_en-US.msi.sig',
    'Token Monitor_1.2.0_x64-setup.exe',
    'Token Monitor_1.2.0_x64-setup.exe.sig',
    'Token Monitor.app.tar.gz',
    'Token Monitor.app.tar.gz.sig'
  ];
  const signatures = {
    'Token Monitor.app.tar.gz.sig': 'darwin-sig',
    'Token Monitor_1.2.0_x64-setup.exe.sig': 'exe-sig'
  };
  const latest = buildLatestJson({
    files,
    version: '1.2.0',
    tag: 'v1.2.0',
    repo: 'MmmmrJ/token-monitor',
    readSignature: (name) => signatures[name] || '',
    now: new Date('2026-07-27T02:00:00.000Z')
  });

  assert.equal(latest.version, '1.2.0');
  assert.match(latest.platforms['darwin-aarch64'].url, /\.app\.tar\.gz$/);
  assert.match(latest.platforms['windows-x86_64'].url, /-setup\.exe$/);
  assert.doesNotMatch(latest.platforms['windows-x86_64'].url, /\.msi$/);
  assert.equal(latest.platforms['windows-x86_64'].signature, 'exe-sig');
  assert.equal(latest.platforms['darwin-x86_64'].signature, 'darwin-sig');
});

test('latest.json prefers nsis.zip when v1Compatible artifacts are present', () => {
  const files = [
    'Token Monitor_1.2.0_x64-setup.exe',
    'Token Monitor_1.2.0_x64-setup.exe.sig',
    'Token Monitor_1.2.0_x64-setup.nsis.zip',
    'Token Monitor_1.2.0_x64-setup.nsis.zip.sig',
    'Token Monitor.app.tar.gz',
    'Token Monitor.app.tar.gz.sig'
  ];
  const latest = buildLatestJson({
    files,
    version: '1.2.0',
    tag: 'v1.2.0',
    repo: 'MmmmrJ/token-monitor',
    readSignature: (name) => `${name}-sig`,
    now: new Date('2026-07-27T02:00:00.000Z')
  });
  assert.match(latest.platforms['windows-x86_64'].url, /\.nsis\.zip$/);
});

test('latest.json rejects missing updater signatures', () => {
  assert.throws(
    () => buildLatestJson({
      files: ['Token Monitor.app.tar.gz', 'Token Monitor_1.2.0_x64-setup.exe'],
      version: '1.2.0',
      tag: 'v1.2.0',
      repo: 'MmmmrJ/token-monitor',
      readSignature: () => 'sig'
    }),
    /missing signature/
  );
});
