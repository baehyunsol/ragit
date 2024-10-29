from add_and_rm import add_and_rm
from end_to_end import end_to_end
from external_bases import external_bases
import random
import sys
from utils import clean

help_message = """
Commands
    end_to_end [model=dummy]    run `end_to_end` test

    external_bases              run `external_bases` test

    add_and_rm                  run `add_and_rm` test

    all [model=dummy]           run all tests
"""

if __name__ == "__main__":
    command = sys.argv[1] if len(sys.argv) > 1 else None
    test_model = sys.argv[2] if len(sys.argv) > 2 else "dummy"
    random.seed(0)

    try:
        if command == "end_to_end":
            end_to_end(test_model=test_model)

        elif command == "external_bases":
            external_bases()

        elif command == "add_and_rm":
            add_and_rm()

        elif command == "all":
            end_to_end(test_model=test_model)
            external_bases()
            add_and_rm()

        else:
            print(help_message)

    finally:
        clean()