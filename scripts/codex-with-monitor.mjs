import { spawn } from 'node:child_process';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const commandArgs = process.argv.slice(2);

function run(command, args, options = {}) {
  return spawn(command, args, { stdio: 'ignore', ...options });
}

function waitForExit(child) {
  return new Promise((resolve) => {
    child.once('error', () => resolve(false));
    child.once('exit', (code) => resolve(code === 0));
  });
}

async function launchInstalledMonitor() {
  if (process.platform === 'darwin') {
    for (const appName of ['Token Monitor', 'Codex Usage Monitor']) {
      if (await waitForExit(run('open', ['-a', appName]))) return true;
    }
    return false;
  }
  if (process.platform === 'win32') {
    const command = [
      "$roots = @($env:APPDATA, $env:ProgramData) | ForEach-Object { Join-Path $_ 'Microsoft\\Windows\\Start Menu\\Programs' }",
      "$names = @('Token Monitor.lnk', 'Codex Usage Monitor.lnk')",
      "$shortcut = Get-ChildItem -Path $roots -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $names -contains $_.Name } | Select-Object -First 1",
      "if ($shortcut) { Start-Process $shortcut.FullName; exit 0 }",
      "$binary = Get-Command 'codex-usage-monitor.exe' -ErrorAction SilentlyContinue",
      "if ($binary) { Start-Process $binary.Source; exit 0 }",
      "exit 1"
    ].join('; ');
    return waitForExit(run('powershell.exe', ['-NoProfile', '-Command', command], { windowsHide: true }));
  }
  return waitForExit(run('codex-usage-monitor', []));
}

function launchDevelopmentMonitor() {
  const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  const child = run(npm, ['run', 'tauri:dev'], { cwd: projectRoot, detached: true });
  child.unref();
}

async function ensureMonitor() {
  if (!await launchInstalledMonitor()) launchDevelopmentMonitor();
}

function printUsage() {
  console.log('Usage: npm run codex -- [Codex arguments]');
  console.log('Launches the native Token Monitor, then starts Codex CLI.');
  console.log('Use --monitor-only to launch only the monitor.');
}

async function main() {
  if (commandArgs.includes('--help') || commandArgs.includes('-h')) {
    printUsage();
    return;
  }

  await ensureMonitor();
  if (commandArgs.includes('--monitor-only')) return;

  const codex = spawn(process.platform === 'win32' ? 'codex.cmd' : 'codex', commandArgs, { stdio: 'inherit' });
  codex.on('error', (error) => {
    console.error(`Could not start Codex: ${error.message}`);
    console.error('Install the Codex CLI or run it from a Codex-enabled terminal.');
    process.exitCode = 1;
  });
  codex.on('exit', (code, signal) => { process.exitCode = signal ? 1 : (code ?? 0); });
}

main();
