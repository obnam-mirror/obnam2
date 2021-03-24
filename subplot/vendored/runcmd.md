# Introduction

The [Subplot][] library `runcmd` for Python provides scenario steps
and their implementations for running Unix commands and examining the
results. The library consists of a bindings file `lib/runcmd.yaml` and
implementations in Python in `lib/runcmd.py`. There is no Bash
version.

[Subplot]: https://subplot.liw.fi/

This document explains the acceptance criteria for the library and how
they're verified. It uses the steps and functions from the
`lib/runcmd` library. The scenarios all have the same structure: run a
command, then examine the exit code, standard output (stdout for
short), or standard error output (stderr) of the command.

The scenarios use the Unix commands `/bin/true` and `/bin/false` to
generate exit codes, and `/bin/echo` to produce stdout. To generate
stderr, they use the little helper script below.

~~~{#err.sh .file .sh .numberLines}
#!/bin/sh
echo "$@" 1>&2
~~~

# Check exit code

These scenarios verify the exit code. To make it easier to write
scenarios in language that flows more naturally, there are a couple of
variations.

## Successful execution

~~~scenario
when I run /bin/true
then exit code is 0
and command is successful
~~~

## Failed execution

~~~scenario
when I try to run /bin/false
then exit code is not 0
and command fails
~~~

# Check we can prepend to $PATH

This scenario verifies that we can add a directory to the beginning of
the PATH environment variable, so that we can have `runcmd` invoke a
binary from our build tree rather than from system directories. This
is especially useful for testing new versions of software that's
already installed on the system.

~~~scenario
given executable script ls from ls.sh
when I prepend . to PATH
when I run ls
then command is successful
then stdout contains "custom ls, not system ls"
~~~

~~~{#ls.sh .file .sh .numberLines}
#!/bin/sh
echo "custom ls, not system ls"
~~~

# Check output has what we want

These scenarios verify that stdout or stderr do have something we want
to have.

## Check stdout is exactly as wanted

Note that the string is surrounded by double quotes to make it clear
to the reader what's inside. Also, C-style string escapes are
understood.

~~~scenario
when I run /bin/echo hello, world
then stdout is exactly "hello, world\n"
~~~

## Check stderr is exactly as wanted

~~~scenario
given helper script err.sh for runcmd
when I run sh err.sh hello, world
then stderr is exactly "hello, world\n"
~~~

## Check stdout using sub-string search

Exact string comparisons are not always enough, so we can verify a
sub-string is in output.

~~~scenario
when I run /bin/echo hello, world
then stdout contains "world\n"
and exit code is 0
~~~

## Check stderr using sub-string search

~~~scenario
given helper script err.sh for runcmd
when I run sh err.sh hello, world
then stderr contains "world\n"
~~~

## Check stdout using regular expressions

Fixed strings are not always enough, so we can verify output matches a
regular expression. Note that the regular expression is not delimited
and does not get any C-style string escaped decoded.

~~~scenario
when I run /bin/echo hello, world
then stdout matches regex world$
~~~

## Check stderr using regular expressions

~~~scenario
given helper script err.sh for runcmd
when I run sh err.sh hello, world
then stderr matches regex world$
~~~

# Check output doesn't have what we want to avoid

These scenarios verify that the stdout or stderr do not
have something we want to avoid.

## Check stdout is not exactly something

~~~scenario
when I run /bin/echo hi
then stdout isn't exactly "hello, world\n"
~~~

## Check stderr is not exactly something

~~~scenario
given helper script err.sh for runcmd
when I run sh err.sh hi
then stderr isn't exactly "hello, world\n"
~~~

## Check stdout doesn't contain sub-string

~~~scenario
when I run /bin/echo hi
then stdout doesn't contain "world"
~~~

## Check stderr doesn't contain sub-string

~~~scenario
given helper script err.sh for runcmd
when I run sh err.sh hi
then stderr doesn't contain "world"
~~~

## Check stdout doesn't match regular expression

~~~scenario
when I run /bin/echo hi
then stdout doesn't match regex world$

~~~

## Check stderr doesn't match regular expressions

~~~scenario
given helper script err.sh for runcmd
when I run sh err.sh hi
then stderr doesn't match regex world$
~~~


---
title: Acceptance criteria for the lib/runcmd Subplot library
author: The Subplot project
template: python
bindings:
- runcmd.yaml
- runcmd_test.yaml
functions:
- runcmd.py
- runcmd_test.py
- files.py
...
