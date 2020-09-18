# Some step implementations for running commands and capturing the result.

import subprocess


# Run a command, capture its stdout, stderr, and exit code in context.
def runcmd(ctx, argv, **kwargs):
    p = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, **kwargs)
    stdout, stderr = p.communicate("")
    ctx["argv"] = argv
    ctx["stdout"] = stdout.decode("utf-8")
    ctx["stderr"] = stderr.decode("utf-8")
    ctx["exit"] = p.returncode


# Check that latest exit code captured by runcmd was a specific one.
def exit_code_is(ctx, wanted):
    if ctx.get("exit") != wanted:
        print("context:", ctx.as_dict())
    assert_eq(ctx.get("exit"), wanted)


# Check that latest exit code captured by runcmd was not a specific one.
def exit_code_is_not(ctx, unwanted):
    if ctx.get("exit") == unwanted:
        print("context:", ctx.as_dict())
    assert_ne(ctx.get("exit"), unwanted)


# Check that latest exit code captured by runcmd was zero.
def exit_code_zero(ctx):
    exit_code_is(ctx, 0)


# Check that latest exit code captured by runcmd was not zero.
def exit_code_nonzero(ctx):
    exit_code_is_not(ctx, 0)


# Check that stdout of latest runcmd contains a specific string.
def stdout_contains(ctx, pattern=None):
    stdout = ctx.get("stdout", "")
    if pattern not in stdout:
        print("pattern:", repr(pattern))
        print("stdout:", repr(stdout))
        print("ctx:", ctx.as_dict())
    assert_eq(pattern in stdout, True)


# Check that stdout of latest runcmd does not contain a specific string.
def stdout_does_not_contain(ctx, pattern=None):
    stdout = ctx.get("stdout", "")
    if pattern in stdout:
        print("pattern:", repr(pattern))
        print("stdout:", repr(stdout))
        print("ctx:", ctx.as_dict())
    assert_eq(pattern not in stdout, True)


# Check that stderr of latest runcmd does contains a specific string.
def stderr_contains(ctx, pattern=None):
    stderr = ctx.get("stderr", "")
    if pattern not in stderr:
        print("pattern:", repr(pattern))
        print("stderr:", repr(stderr))
        print("ctx:", ctx.as_dict())
    assert_eq(pattern in stderr, True)


# Check that stderr of latest runcmd does not contain a specific string.
def stderr_does_not_contain(ctx, pattern=None):
    stderr = ctx.get("stderr", "")
    if pattern not in stderr:
        print("pattern:", repr(pattern))
        print("stderr:", repr(stderr))
        print("ctx:", ctx.as_dict())
    assert_eq(pattern not in stderr, True)
