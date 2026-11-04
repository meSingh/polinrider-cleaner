// The day a page last changed, from git, as Starlight showed it: the date of
// the last commit that touched the file. The deploy checkout is shallow, so
// there every page shows the date of the commit the site was built from.
// Outside a git checkout, the day of the build.
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
