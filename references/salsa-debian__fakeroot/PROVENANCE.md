# clint/fakeroot (salsa.debian.org)

Fetched 2026-09-08T03:17:36Z by hand, not by `scripts/common/mine-repo.sh`.

| | |
| --- | --- |
| commit | `860de25c31d648e04a8264489f924cc18a4717b5` |
| route | direct `git clone --depth 1 https://salsa.debian.org/clint/fakeroot.git` |
| control | n/a |

⛔ **Cite this commit beside every line reference taken from** `tree/`.

## Gaps

- `scripts/common/mine-repo.sh` reaches GitHub only. fakeroot is not on
  GitHub: `paper_final.md` §13 links the Debian package tracker, and the
  source is a GitLab instance at salsa.debian.org. The tree was cloned
  directly, with the commit captured before `.git` was stripped, in the order
  `docs/methodology/references.md` section 1 requires.
- **api/: NOT FETCHED.** No issues, pull requests, comments, review comments,
  releases or tags. The GitLab instance has its own API and
  `scripts/common/mine-repo.sh` does not speak it. fakeroot's defect tracker is
  the Debian BTS at `https://bugs.debian.org/cgi-bin/pkgreport.cgi?pkg=fakeroot`,
  which is a third system again. ⛔ The tracker pass of
  `docs/methodology/references.md` section 3 was **not taken** for this
  reference, and every verdict resting on it is code-only.
