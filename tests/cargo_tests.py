import os
import subprocess
from utils import goto_root

# returns error messages, if exists
def run_cargo_test(location: str) -> list[str]:
    errors = []

    for action in [
        ["cargo", "test"],
        ["cargo", "test", "--release"],
        ["cargo", "doc"],
    ]:
        print(f"running `{' '.join(action)}` at `{location}`")
        result = subprocess.run(action, capture_output=True, text=True)

        if result.returncode != 0:
            errors.append(f"""
#####################
### path: command ###
{os.getcwd()}: {' '.join(action)}

### status_code ###
{result.returncode}

### stdout ###
{result.stdout}

### stderr ###
{result.stderr}
""")

    return errors

def cargo_tests():
    goto_root()
    errors = run_cargo_test("core")
    os.chdir("crates")

    for crate in ["api", "cli", "fs", "ignore", "korean", "pdl", "server"]:
        os.chdir(crate)
        errors += run_cargo_test(crate)
        os.chdir("..")

    if len(errors) > 0:
        raise Exception("\n\n".join(errors))
