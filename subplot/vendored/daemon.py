import logging
import os
import signal
import socket
import subprocess
import time


# A helper function for testing lib/daemon itself.
def _daemon_shell_script(ctx, filename=None):
    get_file = globals()["get_file"]
    data = get_file(filename)
    with open(filename, "wb") as f:
        f.write(data)
    os.chmod(filename, 0o755)


# Start a daemon that will open a port on localhost.
def daemon_start_on_port(ctx, path=None, args=None, name=None, port=None):
    _daemon_start(ctx, path=path, args=args, name=name)
    daemon_wait_for_port("localhost", port)


# Start a daemon after a little wait. This is used only for testing the
# port-waiting code.
def _daemon_start_soonish(ctx, path=None, args=None, name=None, port=None):
    _daemon_start(ctx, path=os.path.abspath(path), args=args, name=name)
    daemon = ctx.declare("_daemon")

    # Store the PID of the process we just started so that _daemon_stop_soonish
    # can kill it during the cleanup phase. This works around the Subplot
    # Python template not giving the step captures to cleanup functions. Note
    # that this code assume at most one _soonish function is called.
    daemon["_soonish"] = daemon[name]["pid"]

    try:
        daemon_wait_for_port("localhost", port)
    except Exception as e:
        daemon["_start_error"] = repr(e)

    logging.info("pgrep: %r", _daemon_pgrep(path))


def _daemon_stop_soonish(ctx, path=None, args=None, name=None, port=None):
    ns = ctx.declare("_daemon")
    pid = ns["_soonish"]
    logging.debug(f"Stopping soonishly-started daemon, {pid}")
    signo = signal.SIGKILL
    try:
        os.kill(pid, signo)
    except ProcessLookupError:
        logging.warning("Process did not actually exist (anymore?)")


# Start a daeamon, get its PID. Don't wait for a port or anything. This is
# meant for background processes that don't have port. Useful for testing the
# lib/daemon library of Subplot, but not much else.
def _daemon_start(ctx, path=None, args=None, name=None):
    runcmd_run = globals()["runcmd_run"]
    runcmd_exit_code_is = globals()["runcmd_exit_code_is"]
    runcmd_get_exit_code = globals()["runcmd_get_exit_code"]
    runcmd_get_stderr = globals()["runcmd_get_stderr"]
    runcmd_prepend_to_path = globals()["runcmd_prepend_to_path"]

    path = os.path.abspath(path)
    argv = [path] + args.split()

    logging.debug(f"Starting daemon {name}")
    logging.debug(f"  ctx={ctx.as_dict()}")
    logging.debug(f"  name={name}")
    logging.debug(f"  path={path}")
    logging.debug(f"  args={args}")
    logging.debug(f"  argv={argv}")

    ns = ctx.declare("_daemon")

    this = ns[name] = {
        "pid-file": f"{name}.pid",
        "stderr": f"{name}.stderr",
        "stdout": f"{name}.stdout",
    }

    # Debian installs `daemonize` to /usr/sbin, which isn't part of the minimal
    # environment that Subplot sets up. So we add /usr/sbin to the PATH.
    runcmd_prepend_to_path(ctx, "/usr/sbin")
    runcmd_run(
        ctx,
        [
            "daemonize",
            "-c",
            os.getcwd(),
            "-p",
            this["pid-file"],
            "-e",
            this["stderr"],
            "-o",
            this["stdout"],
        ]
        + argv,
    )

    # Check that daemonize has exited OK. If it hasn't, it didn't start the
    # background process at all. If so, log the stderr in case there was
    # something useful there for debugging.
    exit = runcmd_get_exit_code(ctx)
    if exit != 0:
        stderr = runcmd_get_stderr(ctx)
        logging.error(f"daemon {name} stderr: {stderr}")
    runcmd_exit_code_is(ctx, 0)

    # Get the pid of the background process, from the pid file created by
    # daemonize. We don't need to wait for it, since we know daemonize already
    # exited. If it isn't there now, it's won't appear later.
    if not os.path.exists(this["pid-file"]):
        raise Exception("daemonize didn't create a PID file")

    this["pid"] = _daemon_wait_for_pid(this["pid-file"], 10.0)

    logging.debug(f"Started daemon {name}")
    logging.debug(f"  pid={this['pid']}")
    logging.debug(f"  ctx={ctx.as_dict()}")


def _daemon_wait_for_pid(filename, timeout):
    start = time.time()
    while time.time() < start + timeout:
        with open(filename) as f:
            data = f.read().strip()
            if data:
                return int(data)
    raise Exception("daemonize created a PID file without a PID")


def daemon_wait_for_port(host, port, timeout=5.0):
    addr = (host, port)
    until = time.time() + timeout
    while True:
        try:
            s = socket.create_connection(addr, timeout=timeout)
            s.close()
            return
        except socket.timeout:
            logging.error(
                f"daemon did not respond at port {port} within {timeout} seconds"
            )
            raise
        except socket.error as e:
            logging.info(f"could not connect to daemon at {port}: {e}")
            pass
        if time.time() >= until:
            logging.error(
                f"could not connect to daemon at {port} within {timeout} seconds"
            )
            raise ConnectionRefusedError()
        # Sleep a bit to avoid consuming too much CPU while busy-waiting.
        time.sleep(0.1)


# Stop a daemon.
def daemon_stop(ctx, path=None, args=None, name=None):
    logging.debug(f"Stopping daemon {name}")

    ns = ctx.declare("_daemon")
    logging.debug(f"  ns={ns}")
    pid = ns[name]["pid"]
    signo = signal.SIGTERM

    this = ns[name]
    data = open(this["stdout"]).read()
    logging.debug(f"{name} stdout, before: {data!r}")
    data = open(this["stderr"]).read()
    logging.debug(f"{name} stderr, before: {data!r}")

    logging.debug(f"Terminating process {pid} with signal {signo}")
    try:
        os.kill(pid, signo)
    except ProcessLookupError:
        logging.warning("Process did not actually exist (anymore?)")

    while True:
        try:
            os.kill(pid, 0)
            logging.debug(f"Daemon {name}, pid {pid} still exists")
            time.sleep(1)
        except ProcessLookupError:
            break
    logging.debug(f"Daemon {name} is gone")

    data = open(this["stdout"]).read()
    logging.debug(f"{name} stdout, after: {data!r}")
    data = open(this["stderr"]).read()
    logging.debug(f"{name} stderr, after: {data!r}")


def daemon_no_such_process(ctx, args=None):
    assert not _daemon_pgrep(args)


def daemon_process_exists(ctx, args=None):
    assert _daemon_pgrep(args)


def _daemon_pgrep(pattern):
    logging.info(f"checking if process exists: pattern={pattern}")
    exit = subprocess.call(
        ["pgrep", "-laf", pattern], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )
    logging.info(f"exit code: {exit}")
    return exit == 0


def daemon_start_fails_with(ctx, message=None):
    daemon = ctx.declare("_daemon")
    error = daemon["_start_error"]
    logging.debug(f"daemon_start_fails_with: error={error!r}")
    logging.debug(f"daemon_start_fails_with: message={message!r}")
    assert message.lower() in error.lower()


def daemon_get_stdout(ctx, name):
    return _daemon_get_output(ctx, name, "stdout")


def daemon_get_stderr(ctx, name):
    return _daemon_get_output(ctx, name, "stderr")


def _daemon_get_output(ctx, name, which):
    ns = ctx.declare("_daemon")
    this = ns[name]
    filename = this[which]
    data = open(filename).read()
    logging.debug(f"Read {which} of daemon {name} from {filename}: {data!r}")
    return data


def daemon_has_produced_output(ctx, name=None):
    started = time.time()
    timeout = 5.0
    while time.time() < started + timeout:
        stdout = daemon_get_stdout(ctx, name)
        stderr = daemon_get_stderr(ctx, name)
        if stdout and stderr:
            break
        time.sleep(0.1)


def daemon_stdout_is(ctx, name=None, text=None):
    daemon_get_stdout = globals()["daemon_get_stdout"]
    _daemon_output_is(ctx, name, text, daemon_get_stdout)


def daemon_stderr_is(ctx, name=None, text=None):
    daemon_get_stderr = globals()["daemon_get_stderr"]
    _daemon_output_is(ctx, name, text, daemon_get_stderr)


def _daemon_output_is(ctx, name, text, getter):
    assert_eq = globals()["assert_eq"]
    text = bytes(text, "UTF-8").decode("unicode_escape")
    output = getter(ctx, name)
    assert_eq(text, output)
