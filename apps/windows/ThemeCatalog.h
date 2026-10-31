#pragma once
#include "pch.h"
#include <map>
#include <vector>
struct EditorPalette {
    winrt::hstring id, name;
    std::map<std::wstring, winrt::Windows::UI::Color> dark, light;
};
struct AppearancePreference { winrt::hstring palette{L"graphite"}, mode{L"system"}; };
class ThemeCatalog final {
    std::vector<EditorPalette> palettes;
public:
    explicit ThemeCatalog(std::filesystem::path const& asset);
    std::vector<EditorPalette> const& Palettes() const { return palettes; }
    EditorPalette const& Palette(winrt::hstring const& id) const;
    AppearancePreference ReadPreference() const;
    void WritePreference(AppearancePreference const& value) const;
};
