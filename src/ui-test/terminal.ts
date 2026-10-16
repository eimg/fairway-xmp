// Deterministic terminal fixture. This module never executes host commands.
type Session = { id: string; directory: string; output: string; line: string; exited: boolean; code: number | null };
const sessions = new Map<string, Session>();
const slots = new Map<number, string>();
let workspace: string | null = null;
const prompt = '\r\n$ ';
export function terminalRequest(command: string, args: Record<string, unknown>, directory: string, version: string, ready: boolean): unknown {
  if (command === 'terminal_context') return { folders: ['hello', 'Student project'], historySupported: directory.startsWith('/') };
  const request = command === 'terminal_start' ? args.request as Record<string, unknown> : args;
  const id = String(request.id);
  if (command === 'terminal_start') {
    if (!ready) throw new Error('Runtime files are missing. Reinstall Fairway XMP to restore PHP and Composer.');
    if (workspace !== request.workspace) { sessions.clear(); slots.clear(); workspace = String(request.workspace); }
    const slot = Number(request.slot);
    const previous = slots.get(slot); if (previous) sessions.delete(previous);
    const cwd = request.folder ? directory + (directory.startsWith('/') ? '/' : '\\') + String(request.folder) : directory;
    sessions.set(id, { id, directory: cwd, output: `Fairway XMP simulated terminal${prompt}`, line: '', exited: false, code: null });
    slots.set(slot, id);
    return { shell: directory.startsWith('/') ? 'zsh' : 'Command Prompt', directory: cwd };
  }
  if (command === 'terminal_close') { sessions.delete(id); for (const [slot, current] of slots) if (current === id) slots.delete(slot); return; }
  const session = sessions.get(id);
  if (!session) throw new Error('Terminal session has ended. Start a new session.');
  if (command === 'terminal_resize') return;
  if (command === 'terminal_read') {
    const data = Array.from(new TextEncoder().encode(session.output)); session.output = '';
    return { data, exited: session.exited, exitCode: session.code, error: null };
  }
  if (command === 'terminal_write') {
    for (const char of String(args.data)) {
      if (char === '\x03') { session.line = ''; session.output += `^C${prompt}`; }
      else if (char === '\x7f') { if (session.line) { session.line = session.line.slice(0, -1); session.output += '\b \b'; } }
      else if (char === '\r' || char === '\n') {
        const line = session.line.trim(); session.line = ''; session.output += '\r\n';
        if (line === 'exit') { session.exited = true; session.code = 0; }
        else {
          if (line === 'pwd' || line === 'cd') session.output += session.directory;
          else if (line === 'php --version') session.output += `PHP ${version} (cli) (simulated)`;
          else if (line === 'composer --version') session.output += 'Composer version 2.10.3 (simulated)';
          else if (line.startsWith('echo ')) session.output += line.slice(5);
          else if (line) session.output += 'UI test only: this command was not executed.';
          session.output += prompt;
        }
      } else if (char >= ' ') { session.line += char; session.output += char; }
    }
    return;
  }
  throw new Error(`Unknown simulated terminal command: ${command}`);
}
