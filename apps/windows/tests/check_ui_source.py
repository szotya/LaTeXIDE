"""Portable structural checks only: this does not compile XAML or WinUI APIs."""
from pathlib import Path
import re
import xml.etree.ElementTree as ET

WINDOWS = Path(__file__).resolve().parents[1]
REPO = WINDOWS.parents[1]
EVENTS = {
    "Click", "TextChanged", "SelectionChanged", "TextCompositionStarted",
    "TextCompositionEnded", "KeyDown", "PreviewKeyDown", "Paste", "Drop",
    "SizeChanged", "ItemInvoked", "TabCloseRequested", "DragDelta",
}
paths = [*WINDOWS.glob("*.xaml"), *WINDOWS.glob("*.vcxproj"),
         *WINDOWS.glob("*.props"), *WINDOWS.glob("*.targets")]
for path in paths:
    ET.parse(path)
window = ET.parse(WINDOWS / "MainWindow.xaml")
handlers = {value for element in window.iter()
            for key, value in element.attrib.items() if key in EVENTS}
header = (WINDOWS / "MainWindow.xaml.h").read_text()
sources = "\n".join(path.read_text() for path in WINDOWS.glob("*.cpp"))
for handler in sorted(handlers):
    assert re.search(r"\b" + handler + r"\s*\(", header), handler
    assert re.search(r"MainWindow::" + handler + r"\s*\(", sources), handler
names = [element.get("{http://schemas.microsoft.com/winfx/2006/xaml}Name")
         for element in window.iter()]
names = [name for name in names if name]
assert len(names) == len(set(names)), "Duplicate XAML names"
resources = {element.get("{http://schemas.microsoft.com/winfx/2006/xaml}Key")
             for element in window.iter()}
for role in re.findall(r"\{ThemeResource (Ide\w+)\}", (WINDOWS / "MainWindow.xaml").read_text()):
    assert role in resources, role
project = ET.parse(WINDOWS / "LaTeXIDE.vcxproj")
for element in project.iter():
    if element.tag.endswith("}ClCompile") and element.get("Include"):
        assert (WINDOWS / element.get("Include")).is_file(), element.get("Include")
asset = REPO / "shared/ide-theme/Sources/IDEThemeAssets/Resources/editor-themes.json"
assert asset.is_file(), asset
assert "editor-themes.json" in (WINDOWS / "LaTeXIDE.vcxproj").read_text()
assert "editor-themes.json" in (REPO / "scripts/build_windows.ps1").read_text()
print(f"PASS: {len(paths)} XML files, {len(handlers)} event handlers, unique names, theme resources and native source/asset references")
print("SCOPE: structure only; Windows XAML generation, C++/WinRT, MSBuild and runtime remain unrun")
