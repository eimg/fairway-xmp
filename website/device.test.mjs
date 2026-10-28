import assert from 'node:assert/strict';
import test from 'node:test';
import { detectPlatform } from './device.js';

const cases = [
  ['Windows x64', { userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)', platform: 'Win32' }, 'windows'],
  ['Mac browser with hidden CPU', { userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)', platform: 'MacIntel' }, 'macos'],
  ['Linux x64', { userAgent: 'Mozilla/5.0 (X11; Linux x86_64)', platform: 'Linux x86_64' }, 'linux'],
  ['client hints platform', { userAgentData: { platform: 'Windows', mobile: false } }, 'windows'],
  ['Android', { userAgent: 'Mozilla/5.0 (Linux; Android 14)' }, null],
  ['iPhone', { userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)' }, null],
  ['desktop-mode iPad', { userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15)', platform: 'MacIntel', maxTouchPoints: 5 }, null],
  ['ChromeOS', { userAgent: 'Mozilla/5.0 (X11; CrOS x86_64)' }, null],
  ['ARM Linux', { userAgent: 'Mozilla/5.0 (X11; Linux aarch64)' }, null],
  ['32-bit Linux', { userAgent: 'Mozilla/5.0 (X11; Linux i686)' }, null],
  ['ARM Windows', { userAgentData: { platform: 'Windows', architecture: 'arm' } }, null],
  ['known Intel Mac', { userAgentData: { platform: 'macOS', architecture: 'x86' } }, null],
  ['unknown browser', {}, null],
];
for (const [name, device, expected] of cases) test(name, () => assert.equal(detectPlatform(device), expected));
