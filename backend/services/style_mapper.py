"""Service for mapping form parameters to StyleOptions.

This service encapsulates the logic for:
1. Resolving a base style from a preset name (optional)
2. Building a StyleOptions instance from a StyleOptionsDTO
"""

from __future__ import annotations

from nectar_render.core.presets import get_builtin_preset
from nectar_render.core.styles import StyleOptions, style_from_option_mapping

from ..schemas.convert_request import StyleOptionsDTO


class StyleMapperService:
    """Maps form parameters (via DTO) to StyleOptions instances."""

    @staticmethod
    def resolve_style(
        style_dto: StyleOptionsDTO,
        preset_name: str | None = None,
    ) -> StyleOptions:
        """Build a StyleOptions from DTO, optionally layering on a preset.

        Args:
            style_dto: The DTO containing style field overrides.
            preset_name: Optional preset name to use as base style.

        Returns:
            A fully resolved StyleOptions instance.

        Raises:
            ValueError: If preset_name is provided but not found.
        """
        base_style: StyleOptions | None = None
        if preset_name:
            base_style = get_builtin_preset(preset_name)
            if base_style is None:
                raise ValueError(f"Unknown preset '{preset_name}'")

        mapping = style_dto.to_mapping()
        return style_from_option_mapping(mapping, base_style=base_style)

    @staticmethod
    def get_preset_or_none(preset_name: str) -> StyleOptions | None:
        """Get a preset by name, returning None if not found.

        Args:
            preset_name: The preset name to look up.

        Returns:
            The preset StyleOptions or None if not found.
        """
        if not preset_name:
            return None
        return get_builtin_preset(preset_name)
