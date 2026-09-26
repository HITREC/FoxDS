import os
import shutil
import zipfile

def make_release():
    base_dir = r"c:\Users\Хитрец\Desktop\tr\перевод_раст"
    proj_dir = os.path.join(base_dir, "VoiceTranslator_Rust")
    stage_dir = os.path.join(base_dir, "FoxDS_VoiceTranslator_Pro_Windows")
    zip_path = os.path.join(base_dir, "FoxDS_VoiceTranslator_Pro_Windows_x64.zip")

    if os.path.exists(stage_dir):
        shutil.rmtree(stage_dir, ignore_errors=True)
    os.makedirs(stage_dir, exist_ok=True)

    files_to_copy = [
        "FoxDS_VoiceTranslator_Rust.exe",
        "WebView2Loader.dll",
        "libunwind.dll",
        "config.json",
        "app_icon.ico",
    ]

    for fname in files_to_copy:
        src = os.path.join(proj_dir, fname)
        dst = os.path.join(stage_dir, fname)
        if os.path.exists(src):
            shutil.copy2(src, dst)
            print(f"Copied: {fname}")
        else:
            print(f"Warning: {src} not found!")

    bat_content = '@echo off\r\nstart "" "%~dp0FoxDS_VoiceTranslator_Rust.exe"\r\n'
    with open(os.path.join(stage_dir, "Запустить_FoxDS_Pro.bat"), "w", encoding="cp866") as f:
        f.write(bat_content)
    with open(os.path.join(stage_dir, "Run_FoxDS_Pro.bat"), "w", encoding="ascii") as f:
        f.write(bat_content)

    readme_content = """=== FoxDS Voice Translator Pro v3.0 (Pure Rust) ===

1. Распакуйте эту папку в любое удобное место.
2. Запустите 'Запустить_FoxDS_Pro.bat' (или FoxDS_VoiceTranslator_Rust.exe).
3. В игре (например Foxhole) или Discord выберите микрофон: 'CABLE Input (VB-Audio Virtual Cable)'.
4. Зажимайте [F4] для голосового перевода рации.
5. Нажимайте [Alt+Q] для экранного снайпера (Win+Shift+S) и перевода текста с экрана!
"""
    with open(os.path.join(stage_dir, "README_ИНСТРУКЦИЯ.txt"), "w", encoding="utf-8") as f:
        f.write(readme_content)
    with open(os.path.join(stage_dir, "README_INSTRUCTION.txt"), "w", encoding="utf-8") as f:
        f.write(readme_content)

    if os.path.exists(zip_path):
        os.remove(zip_path)

    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        for root, dirs, files in os.walk(stage_dir):
            for file in files:
                full_path = os.path.join(root, file)
                rel_path = os.path.relpath(full_path, stage_dir)
                zf.write(full_path, arcname=os.path.join("FoxDS_VoiceTranslator_Pro", rel_path))

    size_mb = os.path.getsize(zip_path) / (1024 * 1024)
    print(f"Created bundle: {zip_path} ({size_mb:.2f} MB)")

if __name__ == "__main__":
    make_release()
