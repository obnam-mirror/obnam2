import logging
import os
import random


def create_file_with_random_data(ctx, filename=None):
    N = 128
    data = "".join(chr(random.randint(0, 255)) for i in range(N)).encode("UTF-8")
    dirname = os.path.dirname(filename) or "."
    logging.debug(f"create_file_with_random_data: dirname={dirname}")
    os.makedirs(dirname, exist_ok=True)
    with open(filename, "wb") as f:
        f.write(data)


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


def files_match(ctx, first=None, second=None):
    assert_eq = globals()["assert_eq"]

    f = open(first).read()
    s = open(second).read()
    logging.debug(f"files_match: f:\n{f}")
    logging.debug(f"files_match: s:\n{s}")
    assert_eq(f, s)
