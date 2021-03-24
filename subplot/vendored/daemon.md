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


# Daemon takes a while to open its port

[netcat]: https://en.wikipedia.org/wiki/Netcat

This scenario verifies that if the background process never starts
listening on its port, the daemon library handles that correctly. We
do this by using [netcat][] to start a dummy daemon, after a short
delay. The lib/daemon code will wait for netcat to open its port, by
connecting to the port. It then closes the port, which causes netcat
to terminate.

~~~scenario
given a daemon helper shell script slow-start-daemon.sh
given there is no "slow-start-daemon.sh" process
when I try to start "./slow-start-daemon.sh" as slow-daemon, on port 8888
when I stop background process slow-daemon
then there is no "slow-start-daemon.sh" process
~~~

~~~{#slow-start-daemon.sh .file .sh .numberLines}
#!/bin/bash

set -euo pipefail

sleep 2
netcat -l 8888 > /dev/null
echo OK
~~~

# Daemon never opens the intended port

This scenario verifies that if the background process never starts
listening on its port, the daemon library handles that correctly.

~~~scenario
given there is no "/bin/sleep 12765" process
when I try to start "/bin/sleep 12765" as sleepyhead, on port 8888
then starting daemon fails with "ConnectionRefusedError"
then a process "/bin/sleep 12765" is running
when I stop background process sleepyhead
then there is no "/bin/sleep 12765" process
~~~


# Daemon stdout and stderr are retrievable

Sometimes it's useful for the step functions to be able to retrieve
the stdout or stderr of of the daemon, after it's started, or even
after it's terminated. This scenario verifies that `lib/daemon` can do
that.

~~~scenario
given a daemon helper shell script chatty-daemon.sh
given there is no "chatty-daemon" process
when I start "./chatty-daemon.sh" as a background process as chatty-daemon
when daemon chatty-daemon has produced output
when I stop background process chatty-daemon
then there is no "chatty-daemon" process
then daemon chatty-daemon stdout is "hi there\n"
then daemon chatty-daemon stderr is "hola\n"
~~~

We make for the daemon to exit, to work around a race condition: if
the test program retrieves the daemon's output too fast, it may not
have had time to produce it yet.


~~~{#chatty-daemon.sh .file .sh .numberLines}
#!/bin/bash

set -euo pipefail

trap 'exit 0' TERM

echo hola 1>&2
echo hi there
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
