import { readFileSync } from 'node:fs';

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), 'utf8');
const packageJson = JSON.parse(read('package.json'));
const packageLock = JSON.parse(read('package-lock.json'));
const tauriConfig = JSON.parse(read('src-tauri/tauri.conf.json'));

const cargoTomlPackage = read('src-tauri/Cargo.toml').match(
  /\[package\][\s\S]*?\nversion\s*=\s*"([^"]+)"/
);
if (!cargoTomlPackage) throw new Error('Could not find the root package version in Cargo.toml');

const cargoLockRoot = read('src-tauri/Cargo.lock')
  .split('[[package]]')
  .find((block) => /\nname\s*=\s*"codex-usage-monitor"\s*\n/.test(`\n${block}`));
const cargoLockVersion = cargoLockRoot?.match(/\nversion\s*=\s*"([^"]+)"/)?.[1];
if (!cargoLockVersion) throw new Error('Could not find codex-usage-monitor in Cargo.lock');

const versions = new Map([
  ['package.json', packageJson.version],
  ['package-lock.json', packageLock.version],
  ['package-lock root package', packageLock.packages?.['']?.version],
  ['tauri.conf.json', tauriConfig.version],
  ['Cargo.toml', cargoTomlPackage[1]],
  ['Cargo.lock root package', cargoLockVersion]
]);
const expected = packageJson.version;
const mismatches = [...versions].filter(([, version]) => version !== expected);

if (mismatches.length) {
  const details = [...versions].map(([file, version]) => `${file}: ${version}`).join('\n');
  throw new Error(`Version mismatch; expected ${expected}\n${details}`);
}

console.log(`All application versions match ${expected}.`);
