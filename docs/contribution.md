# Contribute to Ragit

Thanks for helping Ragit! Before contributing to the project, let me remind you the goals of ragit.

1. Ragit has to be easy-to-use.
2. Knowledge-bases have to be easy-to-share.

If you're familiar with Rust, please help us make ragit more useful. If you're not familiar with Rust, you still have tons of ways to contribute to the project. Using ragit and issuing bug reports help us a lot.

## Add new file readers

It's not the easiest but one of the most helpful way to contribute to ragit. By adding a new file reader, ragit can process new types of files (or parse old types better).

You can find file readers in `src/index/file/`. All you have to do is

1. Add a new file to `src/index/file/`.
2. Define a struct and implement `FileReaderImpl` for that.
3. Add your struct to a match statement in `FileReader::new()`.
4. Run some tests.

Reading documents in the definition of `FileReaderImpl` would help you a lot.

## Prompt Engineering

See [prompt engineering.md](./prompt_engineering.md)

## Write Documents

It also helps a lot.

## Issuing Bugs

Opening a github issue helps us a lot. What's even better is reproducing your bug in `tests/`. `tests/add_and_rm.py` is a good example to reference.

1. Write a new test file in a `test/add_and_rm.py`-like way. You'll find many useful functions at `tests/utils.py`.
2. Add your new test case to `tests.py`.

## Run tests

After writing some code, please make sure to run tests. You can find the tests in `tests/tests.py`. The python file itself is an executable that runs tests. Just run `python tests/tests.py all`. Some tests require api keys. For example, `python tests.py images2 gpt-4o-mini` requires an environment variable: `OPENAI_API_KEY`.
