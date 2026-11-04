// The day a page last changed, from git: the date of the last commit that
// touched the file. The Docs workflow checks out the full history
// (fetch-depth: 0) so each page gets its own date; in a shallow clone every
// page would show the date of the commit being built. On a pull request the
// files it changes show the date GitHub made its merge commit. Outside a
// git checkout, the day of the build.
import { execFileSync } from 'node:child_process';

export function lastUpdated(filePath?: string): string {
  const args = ['log', '-1', '--format=%cs', ...(filePath ? ['--', filePath] : [])];
  try {
    const day = execFileSync('git', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
    if (/^\d{4}-\d{2}-\d{2}$/.test(day)) return day;
  } catch {
    // Not a git checkout.
  }
  return new Date().toISOString().slice(0, 10);
}
