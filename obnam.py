import logging
import os
import random
import requests
import shutil
import socket
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

    ctx["url"] = f"https://localhost:{port}"

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
    with open(filename, "wb") as f:
        f.write(data)


def post_file(ctx, filename=None, path=None, header=None, json=None):
    url = f"{ctx['url']}/chunks"
    headers = {header: json}
    data = open(filename, "rb").read()
    _request(ctx, requests.post, url, headers=headers, data=data)


def get_chunk(ctx, var=None):
    chunk_id = ctx["vars"][var]
    url = f"{ctx['url']}/chunks/{chunk_id}"
    _request(ctx, requests.get, url)


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
