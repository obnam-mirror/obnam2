import logging
import os
import yaml


def install_obnam(ctx):
    runcmd_prepend_to_path = globals()["runcmd_prepend_to_path"]
    srcdir = globals()["srcdir"]

    # Add the directory with built Rust binaries to the path.
    runcmd_prepend_to_path(ctx, dirname=os.path.join(srcdir, "target", "debug"))


def uninstall_obnam(ctx):
    runcmd_run = globals()["runcmd_run"]
    runcmd_run(ctx, ["chmod", "-R", "u+rwX", "."])


def configure_client(ctx, filename=None):
    get_file = globals()["get_file"]

    assert ctx.get("server_url") is not None

    config = get_file(filename)
    config = yaml.safe_load(config)
    config["server_url"] = ctx["server_url"]

    logging.debug(f"client config {filename}: {config}")
    dirname = os.path.expanduser("~/.config/obnam")
    if not os.path.exists(dirname):
        os.makedirs(dirname)
    filename = os.path.join(dirname, "obnam.yaml")
    logging.debug(f"configure_client: filename={filename}")
    with open(filename, "w") as f:
        yaml.safe_dump(config, stream=f)


def run_obnam_restore(ctx, genid=None, todir=None):
    runcmd_run = globals()["runcmd_run"]

    genref = ctx["vars"][genid]
    runcmd_run(ctx, ["env", "RUST_LOG=obnam", "obnam", "restore", genref, todir])


def run_obnam_get_chunk(ctx, gen_id=None, todir=None):
    runcmd_run = globals()["runcmd_run"]
    gen_id = ctx["vars"][gen_id]
    logging.debug(f"run_obnam_get_chunk: gen_id={gen_id}")
    runcmd_run(ctx, ["obnam", "get-chunk", gen_id])


def capture_generation_id(ctx, varname=None):
    runcmd_get_stdout = globals()["runcmd_get_stdout"]

    stdout = runcmd_get_stdout(ctx)
    gen_id = "unknown"
    for line in stdout.splitlines():
        if line.startswith("generation-id:"):
            gen_id = line.split()[-1]

    v = ctx.get("vars", {})
    v[varname] = gen_id
    ctx["vars"] = v


def generation_list_contains(ctx, gen_id=None):
    runcmd_stdout_contains = globals()["runcmd_stdout_contains"]
    gen_id = ctx["vars"][gen_id]
    runcmd_stdout_contains(ctx, text=gen_id)


def file_was_new(ctx, filename=None):
    assert_eq = globals()["assert_eq"]
    reason = get_backup_reason(ctx, filename)
    assert_eq(reason, "(new)")


def file_was_changed(ctx, filename=None):
    assert_eq = globals()["assert_eq"]
    reason = get_backup_reason(ctx, filename)
    assert_eq(reason, "(changed)")


def file_was_unchanged(ctx, filename=None):
    assert_eq = globals()["assert_eq"]
    reason = get_backup_reason(ctx, filename)
    assert_eq(reason, "(unchanged)")


def get_backup_reason(ctx, filename):
    runcmd_get_stdout = globals()["runcmd_get_stdout"]
    stdout = runcmd_get_stdout(ctx)
    lines = stdout.splitlines()
    lines = [line for line in lines if filename in line]
    line = lines[0]
    return line.split()[-1]


def stdout_matches_file(ctx, filename=None):
    runcmd_get_stdout = globals()["runcmd_get_stdout"]
    assert_eq = globals()["assert_eq"]
    stdout = runcmd_get_stdout(ctx)
    data = open(filename).read()
    assert_eq(stdout, data)
