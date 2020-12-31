# Introduction

The [Subplot][] library `files` provides scenario steps and their
implementations for managing files on the file system during tests.
The library consists of a bindings file `lib/files.yaml` and
implementations in Python in `lib/files.py`.

[Subplot]: https://subplot.liw.fi/

This document explains the acceptance criteria for the library and how
they're verified. It uses the steps and functions from the `files`
library.

# Create on-disk files from embedded files

Subplot allows the source document to embed test files, and the
`files` library provides steps to create real, on-disk files from
the embedded files.

~~~scenario
given file hello.txt
then file hello.txt exists
and file hello.txt contains "hello, world"
and file other.txt does not exist
given file other.txt from hello.txt
then file other.txt exists
and files hello.txt and other.txt match
and only files hello.txt, other.txt exist
~~~

~~~{#hello.txt .file .numberLines}
hello, world
~~~


# File metadata

These steps create files and manage their metadata.

~~~scenario
given file hello.txt
when I remember metadata for file hello.txt
then file hello.txt has same metadata as before

when I write "yo" to file hello.txt
then file hello.txt has different metadata from before
~~~

# File modification time

These steps manipulate and test file modification times.

~~~scenario
given file foo.dat has modification time 1970-01-02 03:04:05
then file foo.dat has a very old modification time

when I touch file foo.dat
then file foo.dat has a very recent modification time
~~~


# File contents

These steps verify contents of files.

~~~scenario
given file hello.txt
then file hello.txt contains "hello, world"
and file hello.txt matches regex "hello, .*"
and file hello.txt matches regex /hello, .*/
~~~


---
title: Acceptance criteria for the files Subplot library
author: The Subplot project
template: python
bindings:
- files.yaml
functions:
- files.py
...
