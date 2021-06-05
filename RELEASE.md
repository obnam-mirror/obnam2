---
title: Release checklist for Obnam2
...

# Release checklist for Obnam2

Follow these steps to make a release of Obnam2.

* create a `release` branch
* update `NEWS` with changes for the new release
* update the version number everywhere it needs to be updated; use
  [semantic versioning][]
  - `NEWS`
  - `debian/changelog`
  - `Cargo.toml`
* commit everything
* push changes to gitlab, create merge request, merge, pull back to
  local `main` branch
* create a signed, annotated git tag `vX.Y.Z` for version X.Y.Z for
  the release commit
* push tag to `gitlab.com` and `git.liw.fi`
* publish Obnam crate to crates.io.
* announce new release
  - obnam.org blog, possibly other blogs
  - `#obnam` IRC channel
  - Obnam room on Matrix
  - social media

* prepare `main` branch for further development
  - create new branch
  - update version number again by adding `+git` to it
    - add a new entry to `NEWS` with the `+git` version
	- ditto `debian/changelog`
  - commit
  - push to gitlab, create MR, merge, pull back down to local `main`

* continue development

[semantic versioning]: https://semver.org/
