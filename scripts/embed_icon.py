import os
import sys
import struct
import ctypes
from ctypes import wintypes

def embed_icon(exe_path, ico_path):
    if not os.path.exists(exe_path):
        print(f"Error: Executable not found at {exe_path}")
        return False
    if not os.path.exists(ico_path):
        print(f"Error: Icon not found at {ico_path}")
        return False

    with open(ico_path, "rb") as f:
        ico_data = f.read()

    reserved, itype, count = struct.unpack("<HHH", ico_data[:6])
    if reserved != 0 or itype != 1:
        print("Error: Invalid ICO file")
        return False

    kernel32 = ctypes.windll.kernel32

    BeginUpdateResourceW = kernel32.BeginUpdateResourceW
    BeginUpdateResourceW.argtypes = [wintypes.LPCWSTR, wintypes.BOOL]
    BeginUpdateResourceW.restype = wintypes.HANDLE

    UpdateResourceW = kernel32.UpdateResourceW
    UpdateResourceW.argtypes = [
        wintypes.HANDLE,
        wintypes.LPCWSTR,
        wintypes.LPCWSTR,
        wintypes.WORD,
        wintypes.LPVOID,
        wintypes.DWORD,
    ]
    UpdateResourceW.restype = wintypes.BOOL

    EndUpdateResourceW = kernel32.EndUpdateResourceW
    EndUpdateResourceW.argtypes = [wintypes.HANDLE, wintypes.BOOL]
    EndUpdateResourceW.restype = wintypes.BOOL

    handle = BeginUpdateResourceW(exe_path, False)
    if not handle:
        err = kernel32.GetLastError()
        print(f"Error: BeginUpdateResourceW failed with error code {err}")
        return False

    RT_ICON = ctypes.cast(3, wintypes.LPCWSTR)
    RT_GROUP_ICON = ctypes.cast(14, wintypes.LPCWSTR)
    LANG_NEUTRAL = 0

    # Group icon header: reserved, type, count
    grp_header = bytearray(struct.pack("<HHH", 0, 1, count))

    offset = 6
    for i in range(count):
        entry = ico_data[offset : offset + 16]
        offset += 16
        bWidth, bHeight, bColorCount, bReserved, wPlanes, wBitCount, dwBytesInRes, dwImageOffset = struct.unpack(
            "<BBBBHHII", entry
        )

        icon_id = i + 1
        img_bytes = ico_data[dwImageOffset : dwImageOffset + dwBytesInRes]

        # Add RT_ICON resource
        img_buf = ctypes.create_string_buffer(img_bytes, len(img_bytes))
        res_id = ctypes.cast(icon_id, wintypes.LPCWSTR)
        ok = UpdateResourceW(handle, RT_ICON, res_id, LANG_NEUTRAL, img_buf, len(img_bytes))
        if not ok:
            print(f"Warning: Failed to update RT_ICON {icon_id}")

        # Append to group header (GRPICONDIRENTRY has nID instead of dwImageOffset)
        grp_header.extend(
            struct.pack(
                "<BBBBHHIH",
                bWidth,
                bHeight,
                bColorCount,
                bReserved,
                wPlanes,
                wBitCount,
                dwBytesInRes,
                icon_id,
            )
        )

    # Add RT_GROUP_ICON resource with ID 1
    grp_buf = ctypes.create_string_buffer(bytes(grp_header), len(grp_header))
    main_grp_id = ctypes.cast(1, wintypes.LPCWSTR)
    ok = UpdateResourceW(handle, RT_GROUP_ICON, main_grp_id, LANG_NEUTRAL, grp_buf, len(grp_header))
    if not ok:
        print("Warning: Failed to update RT_GROUP_ICON")

    # Also update with ID 101 for standard compatibility
    main_grp_id_101 = ctypes.cast(101, wintypes.LPCWSTR)
    UpdateResourceW(handle, RT_GROUP_ICON, main_grp_id_101, LANG_NEUTRAL, grp_buf, len(grp_header))

    # Commit changes
    if EndUpdateResourceW(handle, False):
        print(f"Successfully embedded {count} icon resolutions into {exe_path}")
        return True
    else:
        err = kernel32.GetLastError()
        print(f"Error: EndUpdateResourceW failed with code {err}")
        return False

if __name__ == "__main__":
    exe = sys.argv[1] if len(sys.argv) > 1 else "FoxDS_VoiceTranslator_Rust.exe"
    ico = sys.argv[2] if len(sys.argv) > 2 else "app_icon.ico"
    embed_icon(exe, ico)
