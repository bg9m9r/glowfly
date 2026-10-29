# git in the Glowfly Flatpak

This notice comes only with the Flatpak bundle of Glowfly
(`glowfly-<version>-x86_64-unknown-linux-gnu.flatpak`), the one package that carries its own
copy of git. Glowfly's other packages use the git already on your system.

## What is included

The bundle includes **git 2.56.0**, built from its unmodified source release: the program
`/app/bin/git` and the programs it runs, under `/app/libexec/git-core`. Glowfly starts it as a
separate program. Nothing in git was changed; the build leaves out gitk, git-gui, translations
and the parts written in Perl and Python.

## Its licence

git is free software, distributed under the **GNU General Public License, version 2**
(GPL-2.0-only). Some of its parts are under the GNU Lesser General Public License, version 2.1,
or under other licences named in their own source files, among them the two libraries it is
built with: reftable (BSD-3-Clause, copyright Google LLC) and sha1dc (MIT, copyright Marc
Stevens and Dan Shumow). The licence texts are installed with the bundle:

- `/app/share/doc/git/COPYING`: the GNU General Public License, version 2, with git's notes on it
- `/app/share/doc/git/LGPL-2.1`: the GNU Lesser General Public License, version 2.1
- `/app/share/doc/git/reftable-LICENSE`: reftable's BSD licence
- `/app/share/doc/git/sha1dc-LICENSE.txt`: sha1dc's MIT licence

Glowfly's own licence (MIT OR Apache-2.0, in `/app/share/doc/glowfly/`) covers Glowfly only, not
git.

## Its source

- The exact source archive the bundle was built from, `git-2.56.0.tar.xz`, is attached to the
  same release as the `.flatpak` file, on <https://github.com/bg9m9r/glowfly/releases>. Its
  SHA-256 is `26c56c296b38c0695b26fa95f475f1d01704d2d38e73465ca30b0b2f5dc789d3`, and the
  release's `SHA256SUMS` lists it.
- It is the archive git's maintainers publish at
  <https://www.kernel.org/pub/software/scm/git/git-2.56.0.tar.xz>.
- How it was built (the build options and the `make` commands) is in
  `packaging/flatpak/io.github.bg9m9r.glowfly.yml` in Glowfly's repository,
  <https://github.com/bg9m9r/glowfly>, at the release's tag.
