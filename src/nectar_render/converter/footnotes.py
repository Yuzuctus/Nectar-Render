from __future__ import annotations

import re
from dataclasses import dataclass
from html import escape


@dataclass(slots=True)
class FootnoteExtractionResult:
    markdown_without_definitions: str
    definitions: dict[str, str]


_DEFINITION_RE = re.compile(r"^\[\^(?P<id>[^\]]+)\]:\s?(?P<text>.*)$")
_REFERENCE_RE = re.compile(r"\[\^(?P<id>[^\]]+)\]")


def _collect_multiline_note(
    lines: list[str], start_index: int, first_line_text: str
) -> tuple[str, int]:
    chunks = [first_line_text.rstrip()]
    index = start_index + 1
    while index < len(lines):
        current_line = lines[index]
        if not current_line.strip():
            chunks.append("")
            index += 1
            continue
        if current_line.startswith("    ") or current_line.startswith("\t"):
            chunks.append(current_line.lstrip())
            index += 1
            continue
        break
    return "\n".join(chunks).strip(), index


def extract_footnote_definitions(markdown_text: str) -> FootnoteExtractionResult:
    lines = markdown_text.splitlines()
    index = 0
    definitions: dict[str, str] = {}
    kept_lines: list[str] = []

    while index < len(lines):
        line = lines[index]
        match = _DEFINITION_RE.match(line)
        if not match:
            kept_lines.append(line)
            index += 1
            continue

        footnote_id = match.group("id").strip()
        note_text, next_index = _collect_multiline_note(
            lines, index, match.group("text")
        )
        definitions[footnote_id] = note_text
        index = next_index

    return FootnoteExtractionResult(
        markdown_without_definitions="\n".join(kept_lines), definitions=definitions
    )


def inject_paged_footnotes(markdown_text: str, enabled: bool) -> str:
    extraction = extract_footnote_definitions(markdown_text)
    markdown_body = extraction.markdown_without_definitions
    ordered_ids: list[str] = []
    ref_index: dict[str, int] = {}

    def replace_ref(match: re.Match[str]) -> str:
        footnote_id = match.group("id").strip()
        note = extraction.definitions.get(footnote_id, "")
        if not enabled:
            return ""
        if not note:
            return ""
        if footnote_id not in ref_index:
            ordered_ids.append(footnote_id)
            ref_index[footnote_id] = len(ordered_ids)

        note_number = ref_index[footnote_id]
        safe_note = escape(note)
        safe_footnote_id = escape(footnote_id, quote=True)
        return (
            f'<sup class="footnote-ref" id="fnref-{safe_footnote_id}">'
            f'<a href="#fn-{safe_footnote_id}">{note_number}</a>'
            f"</sup>"
            f'<span class="footnote" data-footnote-id="{safe_footnote_id}">{safe_note}</span>'
        )

    with_refs = _REFERENCE_RE.sub(replace_ref, markdown_body)
    if not enabled or not ordered_ids:
        return with_refs

    footnotes_items = "".join(
        (
            f'<li id="fn-{escape(note_id, quote=True)}">'
            f"{escape(extraction.definitions.get(note_id, ''))} "
            f'<a href="#fnref-{escape(note_id, quote=True)}">↩</a>'
            f"</li>"
        )
        for note_id in ordered_ids
    )
    footnotes_html = (
        "\n\n"
        '<section class="footnotes-list">'
        "<hr />"
        "<ol>"
        f"{footnotes_items}"
        "</ol>"
        "</section>"
    )
    return with_refs + footnotes_html
