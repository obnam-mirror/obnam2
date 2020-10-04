import logging
import os
import re
import shlex
import subprocess


#
# Helper functions.
#


# Run a command, given an argv and other arguments for subprocess.Popen.
#
# This is meant to be a helper function, not bound directly to a step. The
# stdout, stderr, and exit code are stored in the "_runcmd" namespace in the
# ctx context.
def runcmd_run(ctx, argv, **kwargs):
    ns = ctx.declare("_runcmd")
    env = dict(os.environ)
    pp = ns.get("path-prefix")
    if pp:
        env["PATH"] = pp + ":" + env["PATH"]

    logging.debug(f"runcmd_run")
    logging.debug(f"  argv: {argv}")
    logging.debug(f"  env: {env}")
    p = subprocess.Popen(
        argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env, **kwargs
    )
    stdout, stderr = p.communicate("")
    ns["argv"] = argv
    ns["stdout.raw"] = stdout
    ns["stderr.raw"] = stderr
    ns["stdout"] = stdout.decode("utf-8")
    ns["stderr"] = stderr.decode("utf-8")
    ns["exit"] = p.returncode
    logging.debug(f"  ctx: {ctx}")
    logging.debug(f"  ns: {ns}")


# Step: prepend srcdir to PATH whenever runcmd runs a command.
def runcmd_helper_srcdir_path(ctx):
    srcdir = globals()["srcdir"]
    runcmd_prepend_to_path(ctx, srcdir)


# Step: This creates a helper script.
def runcmd_helper_script(ctx, filename=None):
    get_file = globals()["get_file"]
    with open(filename, "wb") as f:
        f.write(get_file(filename))


#
# Step functions for running commands.
#


def runcmd_prepend_to_path(ctx, dirname=None):
    ns = ctx.declare("_runcmd")
    pp = ns.get("path-prefix", "")
    if pp:
        pp = f"{pp}:{dirname}"
    else:
        pp = dirname
    ns["path-prefix"] = pp


def runcmd_step(ctx, argv0=None, args=None):
    runcmd_try_to_run(ctx, argv0=argv0, args=args)
    runcmd_exit_code_is_zero(ctx)


def runcmd_try_to_run(ctx, argv0=None, args=None):
    argv = [shlex.quote(argv0)] + shlex.split(args)
    runcmd_run(ctx, argv)


#
# Step functions for examining exit codes.
#


def runcmd_exit_code_is_zero(ctx):
    runcmd_exit_code_is(ctx, exit=0)


def runcmd_exit_code_is(ctx, exit=None):
    assert_eq = globals()["assert_eq"]
    ns = ctx.declare("_runcmd")
    assert_eq(ns["exit"], int(exit))


def runcmd_exit_code_is_nonzero(ctx):
    runcmd_exit_code_is_not(ctx, exit=0)


def runcmd_exit_code_is_not(ctx, exit=None):
    assert_ne = globals()["assert_ne"]
    ns = ctx.declare("_runcmd")
    assert_ne(ns["exit"], int(exit))


#
# Step functions and helpers for examining output in various ways.
#


def runcmd_stdout_is(ctx, text=None):
    ns = ctx.declare("_runcmd")
    _runcmd_output_is(ns["stdout"], text)


def runcmd_stdout_isnt(ctx, text=None):
    ns = ctx.declare("_runcmd")
    _runcmd_output_isnt(ns["stdout"], text)


def runcmd_stderr_is(ctx, text=None):
    ns = ctx.declare("_runcmd")
    _runcmd_output_is(ns["stderr"], text)


def runcmd_stderr_isnt(ctx, text=None):
    ns = ctx.declare("_runcmd")
    _runcmd_output_isnt(ns["stderr"], text)


def _runcmd_output_is(actual, wanted):
    assert_eq = globals()["assert_eq"]
    wanted = bytes(wanted, "utf8").decode("unicode_escape")
    logging.debug("_runcmd_output_is:")
    logging.debug(f"  actual: {actual!r}")
    logging.debug(f"  wanted: {wanted!r}")
    assert_eq(actual, wanted)


def _runcmd_output_isnt(actual, wanted):
    assert_ne = globals()["assert_ne"]
    wanted = bytes(wanted, "utf8").decode("unicode_escape")
    logging.debug("_runcmd_output_isnt:")
    logging.debug(f"  actual: {actual!r}")
    logging.debug(f"  wanted: {wanted!r}")
    assert_ne(actual, wanted)


def runcmd_stdout_contains(ctx, text=None):
    ns = ctx.declare("_runcmd")
    _runcmd_output_contains(ns["stdout"], text)


def runcmd_stdout_doesnt_contain(ctx, text=None):
    ns = ctx.declare("_runcmd")
    _runcmd_output_doesnt_contain(ns["stdout"], text)


def runcmd_stderr_contains(ctx, text=None):
    ns = ctx.declare("_runcmd")
    _runcmd_output_contains(ns["stderr"], text)


def runcmd_stderr_doesnt_contain(ctx, text=None):
    ns = ctx.declare("_runcmd")
    _runcmd_output_doesnt_contain(ns["stderr"], text)


def _runcmd_output_contains(actual, wanted):
    assert_eq = globals()["assert_eq"]
    wanted = bytes(wanted, "utf8").decode("unicode_escape")
    logging.debug("_runcmd_output_contains:")
    logging.debug(f"  actual: {actual!r}")
    logging.debug(f"  wanted: {wanted!r}")
    assert_eq(wanted in actual, True)


def _runcmd_output_doesnt_contain(actual, wanted):
    assert_ne = globals()["assert_ne"]
    wanted = bytes(wanted, "utf8").decode("unicode_escape")
    logging.debug("_runcmd_output_doesnt_contain:")
    logging.debug(f"  actual: {actual!r}")
    logging.debug(f"  wanted: {wanted!r}")
    assert_ne(wanted in actual, True)


def runcmd_stdout_matches_regex(ctx, regex=None):
    ns = ctx.declare("_runcmd")
    _runcmd_output_matches_regex(ns["stdout"], regex)


def runcmd_stdout_doesnt_match_regex(ctx, regex=None):
    ns = ctx.declare("_runcmd")
    _runcmd_output_doesnt_match_regex(ns["stdout"], regex)


def runcmd_stderr_matches_regex(ctx, regex=None):
    ns = ctx.declare("_runcmd")
    _runcmd_output_matches_regex(ns["stderr"], regex)


def runcmd_stderr_doesnt_match_regex(ctx, regex=None):
    ns = ctx.declare("_runcmd")
    _runcmd_output_doesnt_match_regex(ns["stderr"], regex)


def _runcmd_output_matches_regex(actual, regex):
    assert_ne = globals()["assert_ne"]
    r = re.compile(regex)
    m = r.search(actual)
    logging.debug("_runcmd_output_matches_regex:")
    logging.debug(f"  actual: {actual!r}")
    logging.debug(f"  regex: {regex!r}")
    logging.debug(f"  match: {m}")
    assert_ne(m, None)


def _runcmd_output_doesnt_match_regex(actual, regex):
    assert_eq = globals()["assert_eq"]
    r = re.compile(regex)
    m = r.search(actual)
    logging.debug("_runcmd_output_doesnt_match_regex:")
    logging.debug(f"  actual: {actual!r}")
    logging.debug(f"  regex: {regex!r}")
    logging.debug(f"  match: {m}")
    assert_eq(m, None)
