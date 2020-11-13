import os
import subprocess
import yaml


def install_obnam(ctx):
    runcmd_prepend_to_path = globals()["runcmd_prepend_to_path"]
    srcdir = globals()["srcdir"]

    # Add the directory with built Rust binaries to the path.
    runcmd_prepend_to_path(ctx, dirname=os.path.join(srcdir, "target", "debug"))


def configure_client(ctx, filename=None):
    get_file = globals()["get_file"]

    assert ctx.get("server_name") is not None
    assert ctx.get("server_port") is not None

    config = get_file(filename)
    config = yaml.safe_load(config)
    config["server_name"] = ctx["server_name"]
    config["server_port"] = ctx["server_port"]

    with open(filename, "w") as f:
        yaml.safe_dump(config, stream=f)


def run_obnam_restore(ctx, filename=None, genid=None, dbname=None, todir=None):
    runcmd_run = globals()["runcmd_run"]

    genid = ctx["vars"][genid]
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
