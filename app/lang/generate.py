#!/usr/bin/env python3
"""Generate gettext .po files for the bundled Slint translations.

Slint defaults every `@tr` string's msgctxt to the *enclosing component name*, so the .po entries
must carry that context or the runtime lookup misses and the raw key renders. We therefore drive
generation from the extractor's .pot (which records every `(msgctxt, msgid)` occurrence) and pull
the translation text for each msgid from strings.toml. A key used in two components yields two .po
entries (same text, different context) automatically.

Workflow: edit translations in strings.toml, then run `python3 lang/generate.py`. It re-extracts
the .pot from the .slint files and rewrites lang/<locale>/LC_MESSAGES/hdm-am-app.po. Fails (non-zero)
if any string is missing in any language, so an incomplete translation never ships.
"""

import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

LANGS = ("en", "ru", "hy")
DOMAIN = "hdm-am-app"
PLURAL_FORMS = "nplurals=2; plural=(n != 1);"

HERE = Path(__file__).resolve().parent
APP = HERE.parent
POT = HERE / "messages.pot"


def find_extractor() -> str:
    found = shutil.which("slint-tr-extractor")
    if found:
        return found
    fallback = Path.home() / ".cargo" / "bin" / "slint-tr-extractor"
    if fallback.exists():
        return str(fallback)
    print("slint-tr-extractor not found (cargo install slint-tr-extractor)", file=sys.stderr)
    raise SystemExit(2)


def unquote(token: str) -> str:
    token = token.strip()
    if token.startswith('"') and token.endswith('"'):
        token = token[1:-1]
    return token.replace('\\"', '"').replace("\\\\", "\\")


def escape(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def extract_pot() -> None:
    slint_files = [str(p) for p in sorted((APP / "ui").glob("*.slint"))]
    subprocess.run(
        [find_extractor(), *slint_files, "-o", str(POT)],
        check=True,
        cwd=APP,
    )


def parse_pot() -> list[tuple[str | None, str]]:
    """Return the list of (msgctxt, msgid) occurrences, skipping the empty-id header."""
    entries: list[tuple[str | None, str]] = []
    msgctxt: str | None = None
    for raw in POT.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if line.startswith('msgctxt "'):
            msgctxt = unquote(line[len("msgctxt "):])
        elif line.startswith('msgid "'):
            msgid = unquote(line[len("msgid "):])
            if msgid:
                entries.append((msgctxt, msgid))
            msgctxt = None
        elif not line:
            msgctxt = None
    return entries


def main() -> int:
    with (HERE / "strings.toml").open("rb") as handle:
        strings = tomllib.load(handle)

    missing = [
        f"{key} -> {lang}"
        for key, langs in strings.items()
        for lang in LANGS
        if not langs.get(lang, "").strip()
    ]
    if missing:
        print("Missing translations:\n  " + "\n  ".join(missing), file=sys.stderr)
        return 1

    extract_pot()
    entries = parse_pot()

    unknown = sorted({msgid for _, msgid in entries if msgid not in strings})
    if unknown:
        print(
            "Keys used in .slint but absent from strings.toml:\n  " + "\n  ".join(unknown),
            file=sys.stderr,
        )
        return 1

    used = {msgid for _, msgid in entries}
    unused = sorted(set(strings) - used)
    if unused:
        print("warning: unused keys in strings.toml: " + ", ".join(unused), file=sys.stderr)

    for lang in LANGS:
        out = HERE / lang / "LC_MESSAGES" / f"{DOMAIN}.po"
        out.parent.mkdir(parents=True, exist_ok=True)
        lines = [
            'msgid ""',
            'msgstr ""',
            f'"Project-Id-Version: {DOMAIN}\\n"',
            '"POT-Creation-Date: \\n"',
            '"PO-Revision-Date: \\n"',
            '"Last-Translator: \\n"',
            f'"Language-Team: {lang}\\n"',
            f'"Language: {lang}\\n"',
            '"MIME-Version: 1.0\\n"',
            '"Content-Type: text/plain; charset=UTF-8\\n"',
            '"Content-Transfer-Encoding: 8bit\\n"',
            f'"Plural-Forms: {PLURAL_FORMS}\\n"',
            "",
        ]
        for msgctxt, msgid in entries:
            if msgctxt is not None:
                lines.append(f'msgctxt "{escape(msgctxt)}"')
            lines.append(f'msgid "{escape(msgid)}"')
            lines.append(f'msgstr "{escape(strings[msgid][lang])}"')
            lines.append("")
        out.write_text("\n".join(lines), encoding="utf-8")
        print(f"wrote {out.relative_to(APP)} ({len(entries)} entries)")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
