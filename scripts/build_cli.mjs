// Build the native Windows console entry point before Tauri collects resources.
// macOS invokes the signed app executable directly; Linux has its own packager.
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

if (process.platform === 'win32') {
  const root = fileURLToPath(new URL('../', import.meta.url));
  const target = process.env.TAURI_ENV_TARGET_TRIPLE || 'x86_64-pc-windows-msvc';
  const result = spawnSync('cargo', ['build', '--release', '--manifest-path', 'src-tauri/Cargo.toml',
    '--no-default-features', '--features', 'cli', '--bin', 'fairway-cli', '--target', target, '--locked'],
  { cwd: root, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
  const output = path.join(root, 'src-tauri/resources/cli');
  mkdirSync(output, { recursive: true });
  copyFileSync(path.join(root, 'src-tauri/target', target, 'release/fairway-cli.exe'), path.join(output, 'fairway-cli.exe'));
}
