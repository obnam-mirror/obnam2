import json
import logging
import os
import random
import re
import requests
import shutil
import urllib3
import yaml


urllib3.disable_warnings()


def start_chunk_server(ctx, env=None):
    daemon_start_on_port = globals()["daemon_start_on_port"]
    srcdir = globals()["srcdir"]

    logging.debug(f"Starting obnam-server")

    for x in ["test.pem", "test.key"]:
        shutil.copy(os.path.join(srcdir, x), x)

    chunks = "chunks"
    if not os.path.exists(chunks):
        os.mkdir(chunks)

    port = random.randint(2000, 30000)
    ctx["config"] = config = {
        "chunks": chunks,
        "tls_key": "test.key",
        "tls_cert": "test.pem",
        "address": f"localhost:{port}",
    }

    server_binary = ctx["server-binary"]

    filename = "config.yaml"
    yaml.safe_dump(config, stream=open(filename, "w"))
    logging.debug(f"Picked randomly port for obnam-server: {config['address']}")

    ctx["server_url"] = f"https://{config['address']}"

    daemon_start_on_port(
        ctx, name="obnam-server", path=server_binary, args=filename, port=port, env=env
    )


def stop_chunk_server(ctx, env=None):
    logging.debug("Stopping obnam-server")
    daemon_stop = globals()["daemon_stop"]
    daemon_stop(ctx, name="obnam-server")


def post_file(ctx, filename=None, path=None, header=None, json=None):
    url = f"{ctx['server_url']}/v1/chunks"
    headers = {header: json}
    data = open(filename, "rb").read()
    _request(ctx, requests.post, url, headers=headers, data=data)


def get_chunk_via_var(ctx, var=None):
    chunk_id = ctx["vars"][var]
    get_chunk_by_id(ctx, chunk_id=chunk_id)


def get_chunk_by_id(ctx, chunk_id=None):
    url = f"{ctx['server_url']}/v1/chunks/{chunk_id}"
    _request(ctx, requests.get, url)


def find_chunks_with_label(ctx, sha=None):
    url = f"{ctx['server_url']}/v1/chunks?label={sha}"
    _request(ctx, requests.get, url)


def delete_chunk_via_var(ctx, var=None):
    chunk_id = ctx["vars"][var]
    delete_chunk_by_id(ctx, chunk_id=chunk_id)


def delete_chunk_by_id(ctx, chunk_id=None):
    url = f"{ctx['server_url']}/v1/chunks/{chunk_id}"
    _request(ctx, requests.delete, url)


def make_chunk_file_be_empty(ctx, chunk_id=None):
    chunk_id = ctx["vars"][chunk_id]
    chunks = ctx["config"]["chunks"]
    logging.debug(f"trying to empty chunk {chunk_id}")
    for (dirname, _, filenames) in os.walk(chunks):
        logging.debug(f"found directory {dirname}, with {filenames}")
        filename = os.path.join(dirname, chunk_id + ".data")
        if os.path.exists(filename):
            logging.debug(f"emptying chunk file {filename}")
            open(filename, "w").close()


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


def server_has_n_chunks(ctx, n=None):
    assert_eq = globals()["assert_eq"]
    n = int(n)
    files = find_files(ctx["config"]["chunks"])
    files = [x for x in files if x.endswith(".data")]
    logging.debug(f"server_has_n_file_chunks: n={n}")
    logging.debug(f"server_has_n_file_chunks: len(files)={len(files)}")
    logging.debug(f"server_has_n_file_chunks: files={files}")
    assert_eq(n, len(files))


def server_stderr_contains(ctx, wanted=None):
    assert_eq = globals()["assert_eq"]
    assert_eq(_server_stderr_contains(ctx, wanted), True)


def server_stderr_doesnt_contain(ctx, wanted=None):
    assert_eq = globals()["assert_eq"]
    assert_eq(_server_stderr_contains(ctx, wanted), False)


def find_files(root):
    for dirname, _, names in os.walk(root):
        for name in names:
            yield os.path.join(dirname, name)


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
        daemon = ctx.declare("_daemon")
        stderr = open(daemon["obnam-server"]["stderr"], "rb").read()
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


def _server_stderr_contains(ctx, wanted):
    daemon_get_stderr = globals()["daemon_get_stderr"]

    wanted = _expand_vars(ctx, wanted)

    stderr = daemon_get_stderr(ctx, "obnam-server")

    logging.debug(f"_server_stderr_contains:")
    logging.debug(f"  wanted: {wanted}")
    logging.debug(f"  stderr: {stderr}")

    return wanted in stderr
