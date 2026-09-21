# Disclaimer

Read this before running anything in this repository against systems you care
about.

## What this is

An independent open source project written and maintained by one person. It is
not a commercial product. It is not affiliated with, endorsed by, or supported
by GitHub, Socket, OpenSSF, JFrog, safedep, OpenSourceMalware, any security
vendor, or any employer past or present. Their work is cited because it is the
public source of the indicators; that citation is not a partnership and implies
no review of this code by them.

## No warranty

The software is provided "as is", without warranty of any kind, express or
implied, including but not limited to the warranties of merchantability, fitness
for a particular purpose and non-infringement. See [LICENSE](LICENSE).

There is no support commitment and no service level. Issues and pull requests
are read when time allows.

## No liability

To the maximum extent permitted by applicable law, the authors and contributors
accept no liability for any claim, damages or other liability, whether in
contract, tort or otherwise, arising from or in connection with the software or
its use. That includes, without limitation, data loss, loss of access to
repositories, service interruption, and business or financial loss.

## You are responsible for what you run it against

- **Authorisation is yours to establish.** Scanning or modifying repositories,
  organizations or machines that you do not own or administer may breach your
  employer's policy, a customer contract, or applicable law. Get permission
  first, in writing if the systems are not yours.
- **The destructive operations are your decision.** `restore.sh --apply`
  force-updates branch references on GitHub. The local checks with `--apply`
  move files on a machine. Both run with your credentials, at your instruction.
  Read the dry run first.
- **Recovery can lose work.** Restoring a branch to an earlier commit orphans
  anything pushed to it afterwards. The tool tells you when that will happen. Act
  on that warning rather than past it.
- **Take your own backups.** Keep the `evidence/` mirrors until you are certain
  the incident is closed.

## What a result does and does not mean

A clean result means the indicators in [`ioc/`](ioc/) were not found. It is not a
certificate, an audit, or an assurance that a system is uncompromised. Signatures
for this campaign rotate, and a variant this tool does not yet recognise will not
be reported.

A finding means a known indicator matched. It is evidence to investigate, not a
determination. False positives are documented in the README and are expected.

## Not advice

Nothing here is legal advice, regulatory advice, or a compliance assessment. This
tool cannot tell you whether you have a notifiable breach, what your disclosure
obligations are, or whether an incident is over. If those questions are live,
involve your own counsel and your own incident response people.

## Reporting

Security issues in this tool: see [SECURITY.md](SECURITY.md). Findings about the
campaign itself belong with the researchers tracking it, linked in the README.
