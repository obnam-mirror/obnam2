---
title: Obnam &mdash; a backup system
...

# Obnam &mdash; a backup system

Obnam2 is a project to develop a backup system.

For installation instructions and a quick start guide, see
[tutorial.md][]. For more details on goals, requirements, and
implementation details, see the [obnam.md][] subplot file.

[tutorial.md]: https://doc.obnam.org/tutorial.html
[obnam.md]: https://doc.obnam.org/obnam.html

# Dependencies for build and test

The up-to-date, tested list of dependencies for building and testing
Obnam are listed in the file [debian/control](debian/control), in
terms of Debian packages, and in [Cargo.toml](Cargo.toml) for Rust.
The Rust dependencies are handled automatically by the Cargo tool on
all platforms. The other dependencies are, not including ones needed
merely for building Debian packages:

* [Rust](https://www.rust-lang.org/tools/install) &mdash; the
  programming implementation. This can be installed via the standard
  Rust installer, `rustup`, or any other way. Obnam does not currently
  specify an explicit minimum version of Rust it requires, but its
  developers use whatever is the current stable version of the
  language.
  
  On Debian, the `build-essential` package also needs to be installed
  to build Rust programs.

* [daemonize](http://software.clapper.org/daemonize/) &mdash; a tool
  for running a command as a daemon in the background; needed for
  testing, so that the Obnam server can be started and stopped by the
  Obnam test suite.

* [SQLite](https://sqlite.org), specifically its development library
  component &mdash; an SQL database engine that stores the whole
  database in a file and can be used as a library rather then run as a
  service.

* [OpenSSL](https://www.openssl.org), specifically its development
  library component known as `libssl-dev` &mdash; a library that
  implments TLS, which Obnam uses for communication between its client
  and server parts.

* [moreutils](https://joeyh.name/code/moreutils/) &mdash; a collection
  of handy utilities, of which the Obnam test suite uses the `chronic`
  tool to hide output of successful commands.

* [pkg-config](http://pkg-config.freedesktop.org) &mdash; a tool for
  managing compile and link time flags; needed so that the OpenSSL
  library can be linked into the Obnam binaries.

* [Python 3](https://www.python.org/),
  [Requests](http://python-requests.org),
  [PYYAML](https://github.com/yaml/pyyaml) &mdash; programming
  language and libraries for it, used by the Obnam test suite.

* [Subplot](https://subplot.liw.fi) &mdash; a tool for documenting
  acceptance criteria and verifying that they are met.

* [TeX Live](http://www.tug.org/texlive/) &mdash; a typesetting system
  for generating PDF versions of documentation. The LaTeX
  implementation and fonts are needed, not the full suite. None of Tex
  Live is needed, if PDFs aren't needed, but `./check` does not
  currently have a way to be told not to generate PDFs.

* [Summain](https://summain.liw.fi) &mdash; a tool for generating
  manifests of files. Used by the Obnam test suite to verify restored
  data matches the original data.

## Legalese

Copyright 2020-2021  Lars Wirzenius

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program. If not, see <http://www.gnu.org/licenses/>.
