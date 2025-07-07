import json
from utils import (
    cargo_run,
    count_images,
    goto_root,
    mk_and_cd_tmp_dir,
    write_string,
)

def web_images(test_model: str):
    goto_root()
    mk_and_cd_tmp_dir()
    write_string("single.md", "sample image 1: ![](https://raw.githubusercontent.com/baehyunsol/ragit/refs/heads/main/tests/images/hello_world.webp)")
    write_string("double.md", "sample image 2: ![](https://raw.githubusercontent.com/baehyunsol/ragit/refs/heads/main/tests/images/hello_world.webp)\nsample image 3: ![](https://raw.githubusercontent.com/baehyunsol/ragit/refs/heads/main/tests/images/hello_world.webp)")

    cargo_run(["init"])
    cargo_run(["add", "single.md", "double.md"])
    cargo_run(["config", "--set", "strict_file_reader", "true"])
    cargo_run(["config", "--set", "summary_after_build", "false"])
    cargo_run(["config", "--set", "model", test_model])
    cargo_run(["build"])
    cargo_run(["check"])

    assert count_images() == 1
    assert count_images(["single.md"]) == 1
    assert count_images(["double.md"]) == 1

    image = json.loads(cargo_run(["ls-images", "--json"], stdout=True))
    extracted_text = image[0]["extracted_text"].lower()
    assert "hello" in extracted_text
    assert "world" in extracted_text
