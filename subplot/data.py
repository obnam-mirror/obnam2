import logging
import os
import random
import subprocess


def create_file_with_random_data(ctx, filename=None):
    N = 128
    data = "".join(chr(random.randint(0, 255)) for i in range(N)).encode("UTF-8")
    dirname = os.path.dirname(filename) or "."
    logging.debug(f"create_file_with_random_data: dirname={dirname}")
    os.makedirs(dirname, exist_ok=True)
    with open(filename, "wb") as f:
        f.write(data)


def live_and_restored_data_match(ctx, live=None, restore=None):
    subprocess.check_call(["diff", "-rq", f"{live}/.", f"{restore}/{live}/."])
