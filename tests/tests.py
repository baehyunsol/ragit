from add_and_rm import add_and_rm
from add_and_rm2 import add_and_rm2
from archive import archive
from cargo_tests import cargo_tests
from cat_file import cat_file
from cli import cli
from clone import clone
from clone2 import clone2
from csv_reader import csv_reader
from empty import empty
from end_to_end import end_to_end
from external_bases import external_bases
from extract_keywords import extract_keywords
from ignore import ignore
from ii import ii
from images import images
from images2 import images2
from images3 import images3
from ls import ls
from many_chunks import many_chunks
from many_jobs import many_jobs
from markdown_reader import markdown_reader
from merge import merge
from meta import meta
from migrate import migrate
from migrate2 import migrate2
from models_init import models_init
from orphan_process import orphan_process
from prompts import prompts
from ragit_api import ragit_api
from recover import recover
from server import server
from subdir import subdir
from symlink import symlink
from tfidf import tfidf
from web_images import web_images
from write_lock import write_lock

from datetime import datetime
import os
from random import seed as rand_seed
import sys
from utils import (
    clean,
    clean_test_output,
    get_commit_hash,
    get_ragit_version,
    goto_root,
)

def get_platform_info() -> dict[str, str]:
    result = {}

    try:
        import subprocess
        result["cargo_version"] = subprocess.run(["cargo", "version"], capture_output=True, text=True, check=True).stdout.strip()

    except Exception as e:
        result["cargo_version"] = f"cannot get cargo_version: {e}"

    try:
        result["rustc_version"] = subprocess.run(["rustc", "--version"], capture_output=True, text=True, check=True).stdout.strip()

    except Exception as e:
        result["rustc_version"] = f"cannot get rustc_version: {e}"

    try:
        import platform
        result["python_version"] = platform.python_version()

    except Exception as e:
        result["python_version"] = f"cannot get python_version: {e}"

    try:
        result["platform"] = platform.platform()

    except Exception as e:
        result["platform"] = f"cannot get platform: {e}"

    return result

help_message = """
Commands
    end_to_end [model=dummy]    run `end_to_end` test
                                It simulates a basic workflow of ragit: init,
                                add, build and query. It runs on a real dataset:
                                the documents of ragit.

    external_bases              run `external_bases` test
                                It creates bunch of knowledge-bases and run
                                `rag merge` on them. It also checks whether `rag tfidf`
                                can successfully retrieve a chunk from multiple knowledge-bases.

    merge                       run `merge` test
                                It's like `external_bases` test, but with `--prefix` option.

    add_and_rm                  run `add_and_rm` test
                                It runs tons of `rag add` and `rag rm` with
                                different options.

    add_and_rm2                 run `add_and_rm2` test
                                Like `add_and_rm`, but it's more focused on `rag rm`.

    ignore                      run `ignore` test
                                It tests whether `rag add` respects `.ragignore` or `.gitignore`.

    archive                     run `archive` test
                                It runs `archive-create` and `archive-extract` and check
                                if the extracted knowledge-base is identical to the original
                                one.

    recover                     run `recover` test
                                It checks whether 1) `rag check` fails on a broken
                                knowledge-base and 2) `rag check --recover` can
                                fix a broken knowledge-base.

    clone                       run `clone` test
                                It creates a knowledge-base, pushes, clones and checks it.
                                It runs a local `ragit-server` in this repository.

    clone2                      run `clone2` test
                                It clones knowledge-bases from ragit.baehyunsol.com.

    server                      run `server` test
                                It sends requests to every endpoint of ragit-server, except the
                                ones that are tested by `clone`, and checks them.

    cli                         run `cli` test
                                It tests whether cli parser can parse the arguments correctly.

    migrate                     run `migrate` test
                                It checks out git to v 0.1.1, creates a knowledge-base,
                                and run `migrate` until the knowledge-base is migrated to
                                the newest version.
                                Since it runs `git checkout`, it may mess up your working
                                tree. If you have uncommitted changes, this test will fail
                                and does not mess up your working tree.

    migrate2                    run `migrate2` test
                                Like `migrate`, but clones knowledge-bases from web instead
                                of creating a mock knowledge-base.

    many_chunks                 run `many_chunks` test
                                It creates a lot of small files and see if ragit can
                                handle the files correctly. It also tests interrupting
                                `rag build`.

    many_jobs [model=dummy] [jobs=999]
                                run `many_jobs` test
                                `rag build` by default runs with many processes, and a
                                multi-process program may introduce many unexpected bugs.
                                It runs `rag build` with many processes and see if it works.
                                You'd better run it on a machine with many cores.

    ls                          run `ls` test
                                It runs `ls-files`, `ls-chunks`, and `tfidf` with bunch
                                of different options.

    meta                        run `meta` test
                                It runs `rag meta`-family commands and see if it works.

    empty [model=dummy]         run `empty` test
                                It sees if ragit can handle an empty file correctly.

    symlink                     run `symlink` test
                                It tests whether ragit can handle symlinks correctly
                                without falling into infinite loops.

    ii                          run `ii` test
                                It creates an inverted index and test it.

    cat_file                    run `cat_file` test

    images                      run `images` test
                                It creates a markdown file with images and check
                                whether the markdown reader can parse the file
                                correctly.

    images2 [model]             run `images2` test
                                It tests whether models can generate image-description
                                files correctly.
                                NOTE: It uses the vision capability of the model.
                                      Make sure that the model has one.

    images3 [model]             run `images3` test
                                Other tests test images in markdown files, but they
                                don't test image file readers. It does.

    web_images [model]          run `web_images` test
                                It tests whether ragit can fetch images from web.

    extract_keywords [model]    run `extract_keywords` test
                                It tests whether `rag extract-keywords` command works.

    orphan_process              run `orphan_process` test
                                It reproduces gh issue #9.
                                https://github.com/baehyunsol/ragit/issues/9

    write_lock                  run `write_lock` test
                                It reproduces gh issue #8.
                                https://github.com/baehyunsol/ragit/issues/9

    markdown_reader             run `markdown_reader` test
                                I have found many bugs in `markdown_reader_v0`. The bugs
                                are reproduced in this test. If you find a new one, please
                                add that to this test.

    csv_reader                  run `csv_reader` test

    prompts [model=dummy]       run `prompts` test
                                It's the smallest set of commands that parses and executes
                                all the `.pdl` files in `prompts/` directory.

    subdir                      run `subdir` test
                                It checks whether `ragit` is smart enough to find `.ragit/`
                                in any directory.

    tfidf                       run `tfidf` test
                                It creates bunch of lorem-ipsum files and see if
                                `rag tfidf` can retrieve files correctly. It also tests
                                tfidf searches on cjk strings.

    ragit_api [model]           run `ragit_api` test
                                It asks "what's your name" to the model. It returns OK
                                if the api call was successful. It doesn't care about the
                                content of the model's response.

    cargo_tests                 run `cargo test` on all the crates

    models_init                 run `models_init` test
                                It tests the initialization of models.json and
                                model selection in api.json.

    all                         run all tests
                                It dumps the test result to `tests/results.json`.
"""

if __name__ == "__main__":
    no_clean = "--no-clean" in sys.argv
    args = [arg for arg in sys.argv if arg != "--no-clean"]
    seed = [arg for arg in args if arg.startswith("--seed=")]

    if len(seed) > 0:
        args = [arg for arg in args if arg not in seed]
        seed = int(seed[0].split("=")[1])

    else:
        now = datetime.now()
        seed = int(f"{now.year}{now.month}{now.day}{now.hour}{now.minute}{now.second}")

    command = args[1] if len(args) > 1 else None
    test_model = args[2] if len(args) > 2 else None
    rand_seed(seed)

    try:
        if command == "end_to_end":
            test_model = test_model or "dummy"
            end_to_end(test_model=test_model)

        elif command == "external_bases":
            external_bases()

        elif command == "merge":
            merge()

        elif command == "add_and_rm":
            add_and_rm()

        elif command == "add_and_rm2":
            add_and_rm2()

        elif command == "ignore":
            ignore()

        elif command == "recover":
            recover()

        elif command == "clone":
            clone()

        elif command == "clone2":
            clone2()

        elif command == "server":
            server()

        elif command == "cli":
            cli()

        elif command == "migrate":
            migrate()

        elif command == "migrate2":
            migrate2()

        elif command == "archive":
            archive()

        elif command == "many_chunks":
            many_chunks()

        elif command == "many_jobs":
            jobs = args[3] if len(args) > 3 else 999
            test_model = test_model if test_model else "dummy"
            many_jobs(test_model=test_model, jobs=jobs)

        elif command == "ls":
            ls()

        elif command == "meta":
            meta()

        elif command == "symlink":
            symlink()

        elif command == "empty":
            test_model = test_model or "dummy"
            empty(test_model)

        elif command == "ii":
            ii()

        elif command == "cat_file":
            cat_file()

        elif command == "images":
            images()

        elif command == "images2":
            if test_model is None or test_model == "dummy":
                print("Please specify which model to run the tests with. You cannot run this test with a dummy model.")
                sys.exit(1)

            images2(test_model=test_model)

        elif command == "images3":
            if test_model is None or test_model == "dummy":
                print("Please specify which model to run the tests with. You cannot run this test with a dummy model.")
                sys.exit(1)

            images3(test_model=test_model)

        elif command == "web_images":
            if test_model is None or test_model == "dummy":
                print("Please specify which model to run the tests with. You cannot run this test with a dummy model.")
                sys.exit(1)

            web_images(test_model=test_model)

        elif command == "extract_keywords":
            if test_model is None:
                print("Please specify which model to run the tests with.")
                sys.exit(1)

            extract_keywords(test_model=test_model)

        elif command == "orphan_process":
            if test_model is None or test_model == "dummy":
                print("Please specify which model to run the tests with. You cannot run this test with a dummy model.")
                sys.exit(1)

            orphan_process(test_model=test_model)

        elif command == "write_lock":
            if test_model is None or test_model == "dummy":
                print("Please specify which model to run the tests with. You cannot run this test with a dummy model.")
                sys.exit(1)

            write_lock(test_model=test_model)

        elif command == "markdown_reader":
            markdown_reader()

        elif command == "csv_reader":
            csv_reader()

        elif command == "prompts":
            test_model = test_model or "dummy"
            prompts(test_model=test_model)

        elif command == "subdir":
            subdir()

        elif command == "tfidf":
            tfidf()

        elif command == "ragit_api":
            if test_model is None:
                print("Please specify which model to run the tests with.")
                sys.exit(1)

            ragit_api(test_model=test_model)

        elif command == "cargo_tests":
            cargo_tests()
            
        elif command == "models_init":
            models_init()

        elif command == "all":
            import json
            import time
            import traceback

            tests = [
                ("cargo_tests", cargo_tests),
                ("add_and_rm", add_and_rm),
                ("add_and_rm2", add_and_rm2),
                ("ignore", ignore),
                ("recover", recover),
                ("clone", clone),
                ("clone2", clone2),
                ("server", server),
                ("cli", cli),
                ("archive", archive),
                ("many_chunks", many_chunks),
                ("many_jobs", many_jobs),
                ("ls", ls),
                ("meta", meta),
                ("symlink", symlink),
                ("ii", ii),
                ("cat_file", cat_file),
                ("images", images),
                ("markdown_reader", markdown_reader),
                ("csv_reader", csv_reader),
                ("subdir", subdir),
                ("tfidf", tfidf),
                ("merge", merge),
                ("external_bases", external_bases),
                ("end_to_end dummy", lambda: end_to_end(test_model="dummy")),
                ("end_to_end llama3.3-70b", lambda: end_to_end(test_model="llama3.3-70b")),
                ("prompts dummy", lambda: prompts(test_model="dummy")),
                ("prompts gpt-4o-mini", lambda: prompts(test_model="gpt-4o-mini")),
                ("prompts claude-3.5-sonnet", lambda: prompts(test_model="claude-3.5-sonnet")),
                ("empty dummy", lambda: empty(test_model="dummy")),
                ("empty llama3.3-70b", lambda: empty(test_model="llama3.3-70b")),
                ("images2 gpt-4o-mini", lambda: images2(test_model="gpt-4o-mini")),
                ("images3 gpt-4o-mini", lambda: images3(test_model="gpt-4o-mini")),
                ("web_images gpt-4o-mini", lambda: web_images(test_model="gpt-4o-mini")),

                # TODO: replace it with haiku when haiku's vision becomes available
                ("images2 claude-3.5-sonnet", lambda: images2(test_model="claude-3.5-sonnet")),

                ("extract_keywords dummy", lambda: extract_keywords(test_model="dummy")),
                ("extract_keywords gpt-4o-mini", lambda: extract_keywords(test_model="gpt-4o-mini")),
                ("orphan_process llama3.3-70b", lambda: orphan_process(test_model="llama3.3-70b")),
                ("write_lock llama3.3-70b", lambda: write_lock(test_model="llama3.3-70b")),
                ("ragit_api command-r", lambda: ragit_api(test_model="command-r")),
                ("models_init", models_init),
                ("migrate", migrate),
                ("migrate2", migrate2),
            ]
            started_at = datetime.now()
            has_error = False
            result = {
                "meta": {
                    "complete": False,
                    "started_at": str(started_at),
                    "commit": get_commit_hash(),
                    "platform": get_platform_info(),
                    "ragit-version": get_ragit_version(),
                    "rand_seed": seed,
                },
                "tests": {},
                "result": {
                    "total": len(tests),
                    "complete": 0,
                    "pass": 0,
                    "fail": 0,
                    "remaining": len(tests),
                },
            }

            with open("result.json", "w") as f:
                f.write(json.dumps(result, indent=4))

            for seq, (name, test) in enumerate(tests):
                try:
                    start = time.time()
                    rand_seed(seed)
                    test()

                except Exception as e:
                    has_error = True
                    result["tests"][name] = {
                        "seq": seq,
                        "pass": False,
                        "error": clean_test_output(str(e) + "\n" + traceback.format_exc()),
                        "elapsed_ms": int((time.time() - start) * 1000),
                    }
                    result["result"]["fail"] += 1

                else:
                    result["tests"][name] = {
                        "seq": seq,
                        "pass": True,
                        "error": None,
                        "elapsed_ms": int((time.time() - start) * 1000),
                    }
                    result["result"]["pass"] += 1

                finally:
                    result["result"]["complete"] += 1
                    result["result"]["remaining"] -= 1

                    if not no_clean:
                        clean()

                    goto_root()
                    os.chdir("tests")

                    with open("result.json", "w") as f:
                        f.write(json.dumps(result, indent=4))

            ended_at = datetime.now()
            result["meta"]["ended_at"] = str(ended_at)
            result["meta"]["elapsed_ms"] = (ended_at - started_at).seconds * 1000 + (ended_at - started_at).microseconds // 1000
            result["meta"]["complete"] = True
            goto_root()
            os.chdir("tests")
            result = json.dumps(result, indent=4)
            print(result)

            with open("result.json", "w") as f:
                f.write(result)

            if has_error:
                sys.exit(1)

        else:
            print(help_message)

    finally:
        if not no_clean:
            clean()
