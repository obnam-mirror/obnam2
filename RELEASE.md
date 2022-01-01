---
title: Release checklist for Obnam
...

# Release checklist for Obnam

Follow these steps to make a release of Obnam.

1. Create a `release` branch.
   - `git checkout -b release`
2. Update dependencies for the crate, and make any needed changes.
   - `cargo outdated -R`
   - `cargo update`
   - `cargo deny check`
3. Review changes in the crate (`git log vX.Y.Y..`). Update the `NEWS.md`
   file with any changes that users of Obnam need to be aware of.
4. Update the crate's `Cargo.toml` with the appropriate version number
   for the new release, following [semantic versioning][].
5. Update `debian/changelog` with a summary of any changes to the
   Debian packaging (it's not necessary to repeat `NEWS.md` here). Use
   the `dch` command to edit the file to get the format right, since
   it's quite finicky.
   - `dch -v X.Y.Z-1 "New release."`
   - `dch "Changed this thing: foo."`
   - `dch -r ""`
6. Make sure everything still works.
   - `./check`
7. Update `Cargo.lock` after version change.
   - `cargo update`
8. Commit any changes.
9. Run `cargo publish --dry-run` and fix any problems.
10. Push to gitlab.com and create a merge request.
11. Wait for GitLab CI to test the changes successfully. Fix any
    problems it finds.

After the above changes have been merged, do the following steps. You
need to have sufficient access to both the gitlab.com project and the
git.liw.fi project. Currently that means only Lars can do this. These
steps can hopefully be automated in the future.

1. Pull down the above changes from GitLab.
2. Create a signed, annotated git tag `vX.Y.Z` for version X.Y.Z for
  the release commit
  - `git tag -sam "Obnam version X.Y.Z" vX.Y.Z`
3. Push tags to `gitlab.com` and `git.liw.fi` (substitute whatever
   names you use for the remotes):
  - `git push --tags gitlab`
  - `git push --tags origin`
4. Wait for Lars's Ick CI to build the release.
5. Publish Obnam crate to crates.io:
  - `cargo publish`
6. Announce new release:
  - obnam.org blog, possibly other blogs
  - `#obnam` IRC channel
  - Obnam room on Matrix
  - social media

[semantic versioning]: https://semver.org/
