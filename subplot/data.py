import json
import logging
import os
import random
import socket
import stat
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


def create_cachedir_tag_in(ctx, dirpath=None):
    filepath = f"{dirpath}/CACHEDIR.TAG"
    logging.debug(f"creating {filepath}")
    os.makedirs(dirpath, exist_ok=True)
    open(filepath, "w").write("Signature: 8a477f597d28d172789f06886806bc55")


def create_nonutf8_filename(ctx, dirname=None):
    filename = "\x88"
    os.mkdir(dirname)
    open(filename, "wb").close()


def chmod_file(ctx, filename=None, mode=None):
    os.chmod(filename, int(mode, 8))


def create_symlink(ctx, linkname=None, target=None):
    os.symlink(target, linkname)


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
    logging.info(r"verifying that {filename} does not exist")
    try:
        exists = os.path.exists(filename)
    except PermissionError:
        exists = False
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


def match_stdout_to_json_file_superset(ctx, filename=None):
    runcmd_get_stdout = globals()["runcmd_get_stdout"]
    assert_eq = globals()["assert_eq"]
    assert_dict_eq = globals()["assert_dict_eq"]

    stdout = runcmd_get_stdout(ctx)
    stdout = json.loads(stdout.strip())
    obj = json.load(open(filename))
    logging.debug(f"match_stdout_to_json_file: stdout={stdout!r}")
    logging.debug(f"match_stdout_to_json_file: file={obj!r}")

    if isinstance(obj, dict):
        stdout = {key: value for key, value in stdout.items() if key in obj}
        assert_dict_eq(obj, stdout)
    elif isinstance(obj, list):
        obj = {"key": obj}
        stdout = {"key": stdout}
        assert_dict_eq(obj, stdout)
        assert_dict_eq(obj, stdout)
    else:
        assert_eq(obj, stdout)


def match_stdout_to_json_file_exactly(ctx, filename=None):
    runcmd_get_stdout = globals()["runcmd_get_stdout"]
    assert_eq = globals()["assert_eq"]
    assert_dict_eq = globals()["assert_dict_eq"]

    stdout = runcmd_get_stdout(ctx)
    stdout = json.loads(stdout.strip())
    obj = json.load(open(filename))
    logging.debug(f"match_stdout_to_json_file: stdout={stdout!r}")
    logging.debug(f"match_stdout_to_json_file: file={obj!r}")

    if isinstance(obj, list):
        obj = {"key": obj}
        stdout = {"key": stdout}
        assert_dict_eq(obj, stdout)
    elif isinstance(obj, dict):
        assert_dict_eq(obj, stdout)
    else:
        assert_eq(obj, stdout)


def manifests_match(ctx, expected=None, actual=None):
    assert_eq = globals()["assert_eq"]
    assert_dict_eq = globals()["assert_dict_eq"]

    logging.debug(f"comparing manifests {expected} and {actual}")

    expected_objs = list(yaml.safe_load_all(open(expected)))
    actual_objs = list(yaml.safe_load_all(open(actual)))

    logging.debug(f"there are {len(expected_objs)} and {len(actual_objs)} objects")

    i = 0
    while expected_objs and actual_objs:
        e = expected_objs.pop(0)
        a = actual_objs.pop(0)

        logging.debug(f"comparing manifest objects at index {i}:")
        logging.debug(f"  expected: {e}")
        logging.debug(f"  actual  : {a}")
        assert_dict_eq(e, a)

        i += 1

    logging.debug(f"remaining expected objecvts: {expected_objs}")
    logging.debug(f"remaining actual objecvts  : {actual_objs}")
    assert_eq(expected_objs, [])
    assert_eq(actual_objs, [])

    logging.debug(f"manifests {expected} and {actual} match")


def file_is_readable_by_owner(ctx, filename=None):
    assert_eq = globals()["assert_eq"]

    st = os.lstat(filename)
    mode = stat.S_IMODE(st.st_mode)
    logging.debug("file mode: %o", mode)
    assert_eq(mode, 0o400)


def file_does_not_contain(ctx, filename=None, pattern=None):
    data = open(filename).read()
    assert pattern not in data


def files_are_different(ctx, filename1=None, filename2=None):
    assert_ne = globals()["assert_ne"]

    data1 = open(filename1, "rb").read()
    data2 = open(filename2, "rb").read()
    assert_ne(data1, data2)


def files_are_identical(ctx, filename1=None, filename2=None):
    assert_eq = globals()["assert_eq"]

    data1 = open(filename1, "rb").read()
    data2 = open(filename2, "rb").read()
    assert_eq(data1, data2)
