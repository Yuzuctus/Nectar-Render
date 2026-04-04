from __future__ import annotations

from bs4 import BeautifulSoup
from bs4.element import NavigableString, Tag


_HEADING_TAGS = {"h1", "h2", "h3", "h4", "h5", "h6"}
_BLOCK_TAGS = {
    "blockquote",
    "div",
    "figure",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ol",
    "p",
    "pre",
    "table",
    "ul",
}


def normalize_image_blocks(soup: BeautifulSoup) -> None:
    for paragraph in list(soup.find_all("p")):
        image_nodes: list[Tag] = []
        paragraph_is_image_only = True

        for child in paragraph.contents:
            if isinstance(child, NavigableString):
                if child.strip():
                    paragraph_is_image_only = False
                    break
                continue

            if not isinstance(child, Tag):
                paragraph_is_image_only = False
                break

            if child.name == "img":
                image_nodes.append(child)
                continue

            if child.name == "br":
                continue

            paragraph_is_image_only = False
            break

        if not paragraph_is_image_only or not image_nodes:
            continue

        if len(image_nodes) == 1:
            classes = [str(name) for name in paragraph.get("class", [])]
            if "image-block" not in classes:
                paragraph["class"] = [*classes, "image-block"]
            continue

        separated_blocks: list[Tag] = []
        for image in image_nodes:
            image.extract()
            block = soup.new_tag("p")
            block["class"] = ["image-block"]
            block.append(image)
            separated_blocks.append(block)

        first_block = separated_blocks[0]
        paragraph.replace_with(first_block)
        previous_block = first_block
        for block in separated_blocks[1:]:
            previous_block.insert_after(block)
            previous_block = block


def _top_level_blocks(soup: BeautifulSoup) -> list[Tag]:
    return [
        child
        for child in soup.contents
        if isinstance(child, Tag) and child.name in _BLOCK_TAGS
    ]


def _classes(tag: Tag) -> list[str]:
    return [str(name) for name in tag.get("class", [])]


def _add_class(tag: Tag, class_name: str) -> None:
    classes = _classes(tag)
    if class_name not in classes:
        tag["class"] = [*classes, class_name]


def _is_page_break(tag: Tag) -> bool:
    return "page-break" in _classes(tag)


def _is_heading(tag: Tag) -> bool:
    return tag.name in _HEADING_TAGS


def _is_short_intro_block(tag: Tag) -> bool:
    if tag.name not in {"p", "blockquote"}:
        return False
    if tag.find(["table", "pre", "img", "ul", "ol", "div"]):
        return False
    return 0 < len(tag.get_text(" ", strip=True)) <= 220


def _is_compact_list(tag: Tag) -> bool:
    if tag.name not in {"ul", "ol"}:
        return False

    items = tag.find_all("li", recursive=False)
    if not items or len(items) > 6:
        return False

    total_text_length = 0
    for item in items:
        if item.find(["ul", "ol", "table", "pre", "blockquote", "img"]):
            return False

        item_text = item.get_text(" ", strip=True)
        if not item_text or len(item_text) > 140:
            return False
        total_text_length += len(item_text)

    return total_text_length <= 360


def apply_pagination_hints(soup: BeautifulSoup) -> None:
    blocks = _top_level_blocks(soup)

    for index, block in enumerate(blocks):
        if _is_page_break(block):
            continue

        previous_block = blocks[index - 1] if index > 0 else None
        if previous_block is not None and not _is_page_break(previous_block):
            if _is_heading(previous_block):
                _add_class(block, "keep-with-prev")
            if _is_short_intro_block(previous_block) and _is_compact_list(block):
                _add_class(block, "keep-with-prev")

        if _is_heading(block):
            _add_class(block, "keep-with-next")

        if _is_compact_list(block):
            _add_class(block, "compact-list")
            _add_class(block, "keep-together")


__all__ = ["apply_pagination_hints", "normalize_image_blocks"]
