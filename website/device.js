// Browsers can identify an OS more reliably than a processor or OS version.
// Always show requirements alongside the suggested installer.
export function detectPlatform({ userAgent = '', platform = '', maxTouchPoints = 0, userAgentData } = {}) {
  const device = `${userAgentData?.platform || platform} ${userAgent}`;
  if (userAgentData?.mobile || /Android|iPhone|iPad|iPod|Windows Phone|CrOS/i.test(device)) return null;
  // iPadOS can advertise a desktop Mac user agent.
  if (/Mac/i.test(device) && maxTouchPoints > 1) return null;
  const arm = /arm|aarch64/i.test(`${userAgentData?.architecture || ''} ${device}`);
  if (/Windows|Win32|Win64/i.test(device)) return arm ? null : 'windows';
  if (/Mac/i.test(device)) return userAgentData?.architecture === 'x86' ? null : 'macos';
  if (/Linux/i.test(device)) return arm || /i[3-6]86/i.test(device) ? null : 'linux';
  return null;
}
