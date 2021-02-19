import json
import logging
import os
import random
import socket
import yaml


def create_file_with_given_data(ctx, filename=None, data=None):
    logging.debug(f"creating file {filename} with {data!r}")
    dirname = os.path.dirname(filename) or "."
    os.makedirs(dirname, exist_ok=True)
    open(filename, "wb").write(data.encode("UTF-8"))


def create_file_with_random_data(ctx, filename=None):
    N = 128
    data = "".join(chr(random.randint(0, 255)) for i in range(N))
    create_file_with_given_data(ctx, filename=filename, data=data)


def create_unix_socket(ctx, filename=None):
    fd = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    fd.bind(filename)


def create_fifo(ctx, filename=None):
    os.mkfifo(filename)


def create_nonutf8_filename(ctx, dirname=None):
    filename = "\x88"
    os.mkdir(dirname)
    open(filename, "wb").close()


def chmod_file(ctx, filename=None, mode=None):
    os.chmod(filename, int(mode, 8))


def create_symlink(ctx, linkname=None, target=None):
    os.symlink(linkname, target)


def create_manifest_of_live(ctx, dirname=None, manifest=None):
    _create_manifest_of_directory(ctx, dirname=dirname, manifest=manifest)


def create_manifest_of_restored(ctx, dirname=None, restored=None, manifest=None):
    _create_manifest_of_directory(
        ctx, dirname=os.path.join(restored, "./" + dirname), manifest=manifest
    )


def _create_manifest_of_directory(ctx, dirname=None, manifest=None):
    runcmd_run = globals()["runcmd_run"]
    runcmd_get_exit_code = globals()["runcmd_get_exit_code"]
    runcmd_get_stdout = globals()["runcmd_get_stdout"]

    logging.info(f"creating manifest for {dirname} in {manifest}")
    runcmd_run(ctx, ["find", "-exec", "summain", "{}", "+"], cwd=dirname)
    assert runcmd_get_exit_code(ctx) == 0
    stdout = runcmd_get_stdout(ctx)
    open(manifest, "w").write(stdout)


def file_is_restored(ctx, filename=None, restored=None):
    filename = os.path.join(restored, "./" + filename)
    exists = os.path.exists(filename)
    logging.debug(f"restored? {filename} {exists}")
    assert exists


def file_is_not_restored(ctx, filename=None, restored=None):
    filename = os.path.join(restored, "./" + filename)
    exists = os.path.exists(filename)
    logging.debug(f"restored? {filename} {exists}")
    assert not exists


def files_match(ctx, first=None, second=None):
    assert_eq = globals()["assert_eq"]

    f = open(first).read()
    s = open(second).read()
    logging.debug(f"files_match: f:\n{f}")
    logging.debug(f"files_match: s:\n{s}")
    assert_eq(f, s)


def convert_yaml_to_json(ctx, yaml_name=None, json_name=None):
    with open(yaml_name) as f:
        obj = yaml.safe_load(f)
    with open(json_name, "w") as f:
        json.dump(obj, f)


def match_stdout_to_json_file(ctx, filename=None):
    runcmd_get_stdout = globals()["runcmd_get_stdout"]
    assert_eq = globals()["assert_eq"]

    stdout = runcmd_get_stdout(ctx)
    stdout = json.loads(stdout.strip())
    obj = json.load(open(filename))
    logging.debug(f"match_stdout_to_json_file: stdout={stdout!r}")
    logging.debug(f"match_stdout_to_json_file: file={obj!r}")

    for key in obj:
        if key not in stdout:
            logging.error(f"{key} not in stdout")
            assert key in stdout

        if stdout[key] != obj[key]:
            logging.error(f"stdout value for key is not what was exptected")
            assert_eq(stdout[key], obj[key])
