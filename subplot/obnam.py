import json
import logging
import os
import random
import re
import requests
import shutil
import socket
import subprocess
import tarfile
import time
import urllib3
import yaml


urllib3.disable_warnings()


def start_chunk_server(ctx):
    start_daemon = globals()["start_daemon"]
    srcdir = globals()["srcdir"]

    logging.debug(f"Starting obnam-server")

    for x in ["test.pem", "test.key"]:
        shutil.copy(os.path.join(srcdir, x), x)

    chunks = "chunks"
    os.mkdir(chunks)

    config = {"chunks": chunks, "tls_key": "test.key", "tls_cert": "test.pem"}
    port = config["port"] = random.randint(2000, 30000)
    filename = "config.yaml"
    yaml.safe_dump(config, stream=open(filename, "w"))
    logging.debug(f"Picked randomly port for obnam-server: {config['port']}")
    ctx["config"] = config

    ctx["server_name"] = "localhost"
    ctx["server_port"] = port
    ctx["url"] = f"http://localhost:{port}"

    start_daemon(ctx, "obnam-server", [_binary("obnam-server"), filename])

    if not port_open("localhost", port, 5.0):
        stderr = open(ctx["daemon"]["obnam-server"]["stderr"]).read()
        logging.debug(f"Stderr from daemon: {stderr!r}")


def stop_chunk_server(ctx):
    logging.debug("Stopping obnam-server")
    stop_daemon = globals()["stop_daemon"]
    stop_daemon(ctx, "obnam-server")


def create_file_with_random_data(ctx, filename=None):
    N = 128
    data = "".join(chr(random.randint(0, 255)) for i in range(N)).encode("UTF-8")
    dirname = os.path.dirname(filename) or "."
    logging.debug(f"create_file_with_random_data: dirname={dirname}")
    os.makedirs(dirname, exist_ok=True)
    with open(filename, "wb") as f:
        f.write(data)


def post_file(ctx, filename=None, path=None, header=None, json=None):
    url = f"{ctx['url']}/chunks"
    headers = {header: json}
    data = open(filename, "rb").read()
    _request(ctx, requests.post, url, headers=headers, data=data)


def get_chunk_via_var(ctx, var=None):
    chunk_id = ctx["vars"][var]
    get_chunk_by_id(ctx, chunk_id=chunk_id)


def get_chunk_by_id(ctx, chunk_id=None):
    url = f"{ctx['url']}/chunks/{chunk_id}"
    _request(ctx, requests.get, url)


def find_chunks_with_sha(ctx, sha=None):
    url = f"{ctx['url']}/chunks?sha256={sha}"
    _request(ctx, requests.get, url)


def delete_chunk_via_var(ctx, var=None):
    chunk_id = ctx["vars"][var]
    delete_chunk_by_id(ctx, chunk_id=chunk_id)


def delete_chunk_by_id(ctx, chunk_id=None):
    url = f"{ctx['url']}/chunks/{chunk_id}"
    _request(ctx, requests.delete, url)


def status_code_is(ctx, status=None):
    assert_eq = globals()["assert_eq"]
    assert_eq(ctx["http.status"], int(status))


def header_is(ctx, header=None, value=None):
    assert_eq = globals()["assert_eq"]
    assert_eq(ctx["http.headers"][header], value)


def remember_json_field(ctx, field=None, var=None):
    v = ctx.get("vars", {})
    v[var] = ctx["http.json"][field]
    ctx["vars"] = v


def body_matches_file(ctx, filename=None):
    assert_eq = globals()["assert_eq"]
    content = open(filename, "rb").read()
    logging.debug(f"body_matches_file:")
    logging.debug(f"  filename: {filename}")
    logging.debug(f"  content: {content!r}")
    logging.debug(f"  body: {ctx['http.raw']!r}")
    assert_eq(ctx["http.raw"], content)


def json_body_matches(ctx, wanted=None):
    assert_eq = globals()["assert_eq"]
    wanted = _expand_vars(ctx, wanted)
    wanted = json.loads(wanted)
    body = ctx["http.json"]
    logging.debug(f"json_body_matches:")
    logging.debug(f"  wanted: {wanted!r} ({type(wanted)}")
    logging.debug(f"  body  : {body!r} ({type(body)}")
    for key in wanted:
        assert_eq(body.get(key, "not.there"), wanted[key])


def back_up_directory(ctx, dirname=None):
    runcmd_run = globals()["runcmd_run"]

    runcmd_run(ctx, ["pgrep", "-laf", "obnam"])

    config = {"server_name": "localhost", "server_port": ctx["config"]["port"]}
    config = yaml.safe_dump(config)
    logging.debug(f"back_up_directory: {config}")
    filename = "client.yaml"
    with open(filename, "w") as f:
        f.write(config)

    tarball = f"{dirname}.tar"
    t = tarfile.open(name=tarball, mode="w")
    t.add(dirname, arcname=".")
    t.close()

    with open(tarball, "rb") as f:
        runcmd_run(ctx, [_binary("obnam-backup"), filename], stdin=f)


def command_is_successful(ctx):
    runcmd_exit_code_is_zero = globals()["runcmd_exit_code_is_zero"]
    runcmd_exit_code_is_zero(ctx)


# Name of Rust binary, debug-build.
def _binary(name):
    srcdir = globals()["srcdir"]
    return os.path.abspath(os.path.join(srcdir, "target", "debug", name))


# Wait for a port to be open
def port_open(host, port, timeout):
    logging.debug(f"Waiting for port localhost:{port} to be available")
    started = time.time()
    while time.time() < started + timeout:
        try:
            socket.create_connection((host, port), timeout=timeout)
            return True
        except socket.error:
            pass
    logging.error(f"Port localhost:{port} is not open")
    return False


# Make an HTTP request.
def _request(ctx, method, url, headers=None, data=None):
    r = method(url, headers=headers, data=data, verify=False)
    ctx["http.status"] = r.status_code
    ctx["http.headers"] = dict(r.headers)
    try:
        ctx["http.json"] = dict(r.json())
    except ValueError:
        ctx["http.json"] = None
    ctx["http.raw"] = r.content
    logging.debug("HTTP request:")
    logging.debug(f"  url: {url}")
    logging.debug(f"  header: {headers!r}")
    logging.debug("HTTP response:")
    logging.debug(f"  status: {r.status_code}")
    logging.debug(f"  json: {ctx['http.json']!r}")
    logging.debug(f"  text: {r.content!r}")
    if not r.ok:
        stderr = open(ctx["daemon"]["obnam-server"]["stderr"], "rb").read()
        logging.debug(f"  server stderr: {stderr!r}")


# Expand variables ("<foo>") in a string with values from ctx.
def _expand_vars(ctx, s):
    v = ctx.get("vars")
    if v is None:
        return s
    result = []
    while True:
        m = re.search(f"<(\\S+)>", s)
        if not m:
            result.append(s)
            break
        result.append(s[: m.start()])
        value = v[m.group(1)]
        result.append(value)
        s = s[m.end() :]
    return "".join(result)


def install_obnam(ctx):
    runcmd_prepend_to_path = globals()["runcmd_prepend_to_path"]
    srcdir = globals()["srcdir"]

    # Add the directory with built Rust binaries to the path.
    runcmd_prepend_to_path(ctx, dirname=os.path.join(srcdir, "target", "debug"))


def configure_client(ctx, filename=None):
    get_file = globals()["get_file"]

    config = get_file(filename)
    ctx["client-config"] = yaml.safe_load(config)


def run_obnam_backup(ctx, filename=None):
    runcmd_run = globals()["runcmd_run"]

    _write_obnam_client_config(ctx, filename)
    runcmd_run(ctx, ["env", "RUST_LOG=obnam", "obnam", "backup", filename])


def run_obnam_list(ctx, filename=None):
    runcmd_run = globals()["runcmd_run"]

    _write_obnam_client_config(ctx, filename)
    runcmd_run(ctx, ["env", "RUST_LOG=obnam", "obnam", "list", filename])


def _write_obnam_client_config(ctx, filename):
    config = ctx["client-config"]
    config["server_name"] = ctx["server_name"]
    config["server_port"] = ctx["server_port"]
    with open(filename, "w") as f:
        yaml.safe_dump(config, stream=f)


def run_obnam_restore(ctx, filename=None, genid=None, dbname=None, todir=None):
    runcmd_run = globals()["runcmd_run"]

    genid = ctx["vars"][genid]
    _write_obnam_client_config(ctx, filename)
    runcmd_run(
        ctx,
        ["env", "RUST_LOG=obnam", "obnam", "restore", filename, genid, dbname, todir],
    )


def capture_generation_id(ctx, varname=None):
    runcmd_get_stdout = globals()["runcmd_get_stdout"]

    stdout = runcmd_get_stdout(ctx)
    gen_id = "unknown"
    for line in stdout.splitlines():
        if line.startswith("gen id:"):
            gen_id = line.split()[-1]

    v = ctx.get("vars", {})
    v[varname] = gen_id
    ctx["vars"] = v


def live_and_restored_data_match(ctx, live=None, restore=None):
    subprocess.check_call(["diff", "-rq", f"{live}/.", f"{restore}/{live}/."])


def generation_list_contains(ctx, gen_id=None):
    runcmd_stdout_contains = globals()["runcmd_stdout_contains"]
    gen_id = ctx["vars"][gen_id]
    runcmd_stdout_contains(ctx, text=gen_id)
