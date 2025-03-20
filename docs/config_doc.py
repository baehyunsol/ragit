import os
import re
from typing import Optional

def goto_root():
    while "Cargo.toml" not in os.listdir() or ".gitignore" not in os.listdir():
        os.chdir("..")

def init():
    goto_root()

# It's NOT a complete parser. Its regices might have to be adjusted when the code changes
def extract_struct(
    haystack: str,
    original_name: str,
    rename_to: Optional[str],
    remove_pub_keywords: bool,
    remove_internal_comments: bool = True,
    remove_derives: bool = True,
) -> str:
    extracted = re.search(r"struct\s*" + original_name + r"\s*\{[^{}]+\}", haystack, re.DOTALL).group(0)

    if rename_to is not None:
        extracted = re.sub(
            r"struct(\s*" + original_name + r"\s*)\{",
            lambda m: f"struct{m.group(1).replace(original_name, rename_to)}" + '{',
            extracted,
        )

    if remove_pub_keywords:
        extracted = re.sub(
            r"^(\s+)pub\s+([a-z0-9_]+.+)$",
            lambda m: m.group(1) + m.group(2),
            extracted,
            flags=re.MULTILINE,
        )

    if remove_internal_comments:
        lines = extracted.split("\n")
        extracted = []

        for line in lines:
            if re.match(r"^\s*//\s+.+", line) is None:
                extracted.append(line)

        extracted = "\n".join(extracted)

    if remove_derives:
        lines = extracted.split("\n")
        extracted = []

        for line in lines:
            if re.match(r"^\s*\#\[.+\].*", line) is None:
                extracted.append(line)

        extracted = "\n".join(extracted)

    return extracted

# It's NOT a complete parser. Its regices might have to be adjusted when the code changes
def extract_default_values(
    haystack: str,
    struct_name: str,
    trim_lines: bool,
    line_prefix: Optional[str],
) -> str:
    extracted = re.search(
        r"impl\s*Default\s*for\s*" + struct_name + r"\s*\{.*" + struct_name + r"\s*\{([^{}]+)\}",
        haystack,
        flags=re.DOTALL,
    ).group(1)

    if trim_lines:
        extracted = "\n".join([line.strip() for line in extracted.split("\n") if line.strip() != ""])

    if line_prefix is not None:
        extracted = "\n".join([line_prefix + line for line in extracted.split("\n")])

    return extracted

def derustify_string(s: str) -> str:
    s = re.sub(
        r"(\d)_(\d)",
        lambda m: m.group(1) + m.group(2),
        s,
    )
    s = re.sub(
        r"Some\(([^()]+)\)",
        lambda m: m.group(1),
        s,
    )
    s = re.sub(
        r"^(.*)String\:\:from\((.+)\)(.*)$",
        lambda m: m.group(1) + m.group(2) + m.group(3),
        s,
        flags=re.MULTILINE,
    )
    return s

def main():
    with open("./src/index/config.rs", "r") as f:
        build_config_file = f.read()

    build_config = extract_struct(
        build_config_file,
        "BuildConfig",
        "BuildConfig",
        True,
    )
    build_defaults = extract_default_values(
        build_config_file,
        "BuildConfig",
        True,
        "// ",
    )

    with open("./src/query/config.rs", "r") as f:
        query_config_file = f.read()

    query_config = extract_struct(
        query_config_file,
        "QueryConfig",
        "QueryConfig",
        True,
    )
    query_defaults = extract_default_values(
        query_config_file,
        "QueryConfig",
        True,
        "// ",
    )

    with open("./src/api_config.rs", "r") as f:
        api_config_file = f.read()

    api_config = extract_struct(
        api_config_file,
        "ApiConfig",
        "ApiConfig",
        True,
    )
    api_defaults = extract_default_values(
        api_config_file,
        "ApiConfig",
        True,
        "// ",
    )

    result =  f"""
// default values
{derustify_string(build_defaults)}
{build_config}

// default values
{derustify_string(query_defaults)}
{query_config}

// default values
{derustify_string(api_defaults)}
{api_config}"""

    with open("./docs/config.md", "r") as f:
        doc_file = f.read()

    doc_file = re.sub(
        r"(```rust\n).*(\n```)",
        lambda m: m.group(1) + result + m.group(2),
        doc_file,
        flags=re.DOTALL,
    )

    with open("./docs/config.md", "w") as f:
        f.write(doc_file)

if __name__ == "__main__":
    init()
    main()
