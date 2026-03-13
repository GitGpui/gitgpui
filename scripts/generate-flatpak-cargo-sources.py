#!/usr/bin/env python3
import argparse
import json
import tomllib
from pathlib import Path

CRATES_IO = "https://static.crates.io/crates"
CARGO_HOME = "cargo"
CARGO_CRATES = f"{CARGO_HOME}/vendor"
VENDORED_SOURCES = "vendored-sources"
CRATES_IO_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"


def package_checksum(package: dict, metadata: dict) -> str:
    checksum = package.get("checksum")
    if checksum:
        return checksum

    key = (
        f'checksum {package["name"]} {package["version"]} '
        f'({package["source"]})'
    )
    checksum = metadata.get(key)
    if checksum:
        return checksum
    raise SystemExit(
        f'missing checksum for {package["name"]} {package["version"]}'
    )


def cargo_config_contents() -> str:
    return (
        "[source.crates-io]\n"
        f'replace-with = "{VENDORED_SOURCES}"\n\n'
        f"[source.{VENDORED_SOURCES}]\n"
        f'directory = "{CARGO_CRATES}"\n'
    )


def generate_sources(lock_path: Path) -> list[dict]:
    with lock_path.open("rb") as handle:
        cargo_lock = tomllib.load(handle)

    metadata = cargo_lock.get("metadata", {})
    seen = set()
    sources = []
    for package in cargo_lock["package"]:
        source = package.get("source")
        if source is None:
            continue
        if source != CRATES_IO_SOURCE:
            raise SystemExit(
                f'unsupported non-crates.io dependency: {package["name"]} {source}'
            )

        key = (package["name"], package["version"])
        if key in seen:
            continue
        seen.add(key)

        checksum = package_checksum(package, metadata)
        dest = f'{CARGO_CRATES}/{package["name"]}-{package["version"]}'
        sources.append(
            {
                "type": "archive",
                "archive-type": "tar-gzip",
                "url": (
                    f'{CRATES_IO}/{package["name"]}/'
                    f'{package["name"]}-{package["version"]}.crate'
                ),
                "sha256": checksum,
                "dest": dest,
            }
        )
        sources.append(
            {
                "type": "inline",
                "contents": json.dumps({"package": checksum, "files": {}}),
                "dest": dest,
                "dest-filename": ".cargo-checksum.json",
            }
        )

    sources.append(
        {
            "type": "inline",
            "contents": cargo_config_contents(),
            "dest": CARGO_HOME,
            "dest-filename": "config",
        }
    )
    return sources


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "cargo_lock",
        nargs="?",
        default="Cargo.lock",
        help="Path to Cargo.lock",
    )
    parser.add_argument(
        "-o",
        "--output",
        default="flatpak/cargo-sources.json",
        help="Path to write generated cargo sources JSON",
    )
    args = parser.parse_args()

    sources = generate_sources(Path(args.cargo_lock))
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(sources, indent=4) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
