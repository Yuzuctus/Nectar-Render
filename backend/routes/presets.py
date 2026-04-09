from __future__ import annotations

from fastapi import APIRouter

from nectar_render.core.presets import (
    BUILTIN_PRESET_NAMES,
    BUILTIN_PRESET_AS_DICTS,
)

router = APIRouter(prefix="/presets", tags=["presets"])


@router.get("/builtin")
async def list_builtin_presets() -> dict[str, object]:
    return {
        "presets": {
            name: {
                "name": name,
                "style": BUILTIN_PRESET_AS_DICTS.get(name, {}),
            }
            for name in BUILTIN_PRESET_NAMES
        },
    }
