"""Ask the Windows Shell for a real folder-background menu (without showing it)."""
import ctypes
from ctypes import wintypes
from pathlib import Path
import sys
import uuid


class GUID(ctypes.Structure):
    _fields_ = [("bytes", ctypes.c_ubyte * 16)]

    def __init__(self, value):
        super().__init__((ctypes.c_ubyte * 16).from_buffer_copy(uuid.UUID(value).bytes_le))


def check_hresult(result):
    if result < 0:
        raise OSError(f"Shell call failed: 0x{result & 0xffffffff:08x}")


def invoke(pointer, slot, result_type, argument_types, *arguments):
    vtable = ctypes.cast(pointer, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p))).contents
    method = ctypes.WINFUNCTYPE(result_type, ctypes.c_void_p, *argument_types)(vtable[slot])
    return method(pointer, *arguments)


def background_labels(folder_path):
    pointer = ctypes.c_void_p
    ole = ctypes.OleDLL("ole32")
    shell = ctypes.OleDLL("shell32")
    user = ctypes.WinDLL("user32")
    ole.CoInitializeEx.argtypes = [pointer, wintypes.DWORD]
    ole.CoTaskMemFree.argtypes = [pointer]
    shell.SHParseDisplayName.argtypes = [
        wintypes.LPCWSTR, pointer, ctypes.POINTER(pointer), wintypes.DWORD, pointer
    ]
    shell.SHGetDesktopFolder.argtypes = [ctypes.POINTER(pointer)]
    user.CreatePopupMenu.restype = pointer
    user.GetMenuItemCount.argtypes = [pointer]
    user.GetMenuStringW.argtypes = [pointer, wintypes.UINT, wintypes.LPWSTR, ctypes.c_int, wintypes.UINT]
    user.DestroyMenu.argtypes = [pointer]
    check_hresult(ole.CoInitializeEx(None, 2))
    pidl, desktop, folder, context_menu = (pointer() for _ in range(4))
    handle = None
    try:
        check_hresult(shell.SHParseDisplayName(str(folder_path), None, ctypes.byref(pidl), 0, None))
        check_hresult(shell.SHGetDesktopFolder(ctypes.byref(desktop)))
        folder_iid = GUID("000214e6-0000-0000-c000-000000000046")
        check_hresult(invoke(
            desktop, 5, ctypes.c_long, [pointer, pointer, ctypes.POINTER(GUID), ctypes.POINTER(pointer)],
            pidl, None, ctypes.byref(folder_iid), ctypes.byref(folder)
        ))
        menu_iid = GUID("000214e4-0000-0000-c000-000000000046")
        # IShellFolder::CreateViewObject requests the background menu;
        # GetUIObjectOf would only test a selected item's menu instead.
        check_hresult(invoke(
            folder, 8, ctypes.c_long, [pointer, ctypes.POINTER(GUID), ctypes.POINTER(pointer)],
            None, ctypes.byref(menu_iid), ctypes.byref(context_menu)
        ))
        handle = user.CreatePopupMenu()
        if not handle:
            raise OSError("Cannot create a popup menu")
        check_hresult(invoke(
            context_menu, 3, ctypes.c_long, [pointer] + [wintypes.UINT] * 4,
            handle, 0, 1, 0x7fff, 0
        ))
        labels = []
        count = user.GetMenuItemCount(handle)
        if count < 0:
            raise OSError("Cannot read the background menu")
        for index in range(count):
            text = ctypes.create_unicode_buffer(1024)
            user.GetMenuStringW(handle, index, text, len(text), 0x400)
            labels.append(text.value.replace("&", ""))
        return labels
    finally:
        if handle:
            user.DestroyMenu(handle)
        for item in (context_menu, folder, desktop):
            if item:
                invoke(item, 2, wintypes.ULONG, [])
        ole.CoTaskMemFree(pidl)
        ole.CoUninitialize()


if __name__ == "__main__":
    labels = background_labels(Path(sys.argv[1]).resolve())
    for expected in ("由二维码发送", "由二维码接收"):
        assert expected in labels, f"Background menu missing {expected!r}: {labels!r}"
    print("PASS: Windows Shell exposes both QR actions in the folder background menu.")
