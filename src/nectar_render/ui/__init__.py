__all__ = ["NectarRenderApp"]


def __getattr__(name: str):
    if name == "NectarRenderApp":
        from ..interfaces.desktop.app import NectarRenderApp

        return NectarRenderApp
    msg = f"module {__name__!r} has no attribute {name!r}"
    raise AttributeError(msg)
