# Introduction

The [Subplot][] library `daemon` for Python provides scenario steps
and their implementations for running a background process and
terminating at the end of the scenario.

[Subplot]: https://subplot.liw.fi/

This document explains the acceptance criteria for the library and how
they're verified. It uses the steps and functions from the
`lib/daemon` library. The scenarios all have the same structure: run a
command, then examine the exit code, verify the process is running.

# Daemon is started and terminated

This scenario starts a background process, verifies it's started, and
verifies it's terminated after the scenario ends.

~~~scenario
given there is no "/bin/sleep 12765" process
when I start "/bin/sleep 12765" as a background process as sleepyhead
then a process "/bin/sleep 12765" is running
when I stop background process sleepyhead
then there is no "/bin/sleep 12765" process
~~~



---
title: Acceptance criteria for the lib/daemon Subplot library
author: The Subplot project
bindings:
- daemon.yaml
template: python
functions:
- daemon.py
- runcmd.py
...
