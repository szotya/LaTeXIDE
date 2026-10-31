#include "ThemeCatalog.h"
#include <ShlObj.h>
#include <fstream>
#include <iterator>
#include <array>
using namespace winrt;
using namespace Windows::Data::Json;
namespace {
constexpr std::array<wchar_t const*, 24> roles{ L"chrome",L"sidebar",L"editor",L"panel",L"border",L"selection",L"currentLine",L"hover",L"text",L"secondary",L"accent",L"success",L"warning",L"error",L"command",L"environment",L"delimiter",L"comment",L"math",L"parameter",L"literal",L"number",L"operator",L"attribute" };
constexpr std::array<wchar_t const*, 11> syntaxRoles{L"text",L"command",L"environment",L"delimiter",L"comment",L"math",L"parameter",L"literal",L"number",L"operator",L"attribute"};
std::string readFile(std::filesystem::path const& path) {
    std::ifstream stream(path,std::ios::binary);
    if(!stream) throw hresult_error(E_FAIL,L"A helyi témafájl nem olvasható: "+hstring(path.native()));
    return {std::istreambuf_iterator<char>(stream), std::istreambuf_iterator<char>()};
}
Windows::UI::Color color(hstring const& value) {
    auto text=std::wstring(value);
    if(text.size()!=7 || text[0]!=L'#' || text.find_first_not_of(L"0123456789ABCDEFabcdef",1)!=std::wstring::npos)
        throw hresult_error(E_INVALIDARG,L"Hibás helyi témaszín.");
    auto number=std::stoul(text.substr(1),nullptr,16);
    return {255,static_cast<uint8_t>(number>>16),static_cast<uint8_t>(number>>8),static_cast<uint8_t>(number)};
}
std::map<std::wstring,Windows::UI::Color> colors(JsonObject const& values) {
    std::map<std::wstring,Windows::UI::Color> result;
    for(auto const* role:roles) result.emplace(role,color(values.GetNamedString(role)));
    return result;
}
std::filesystem::path preferencePath() {
    PWSTR raw{}; check_hresult(SHGetKnownFolderPath(FOLDERID_LocalAppData,KF_FLAG_DEFAULT,nullptr,&raw));
    std::unique_ptr<wchar_t,decltype(&CoTaskMemFree)> owned(raw,CoTaskMemFree);
    return std::filesystem::path(raw)/L"LaTeXIDE"/L"appearance.json";
}
}
ThemeCatalog::ThemeCatalog(std::filesystem::path const& asset) {
    auto root=JsonObject::Parse(to_hstring(readFile(asset)));
    if(root.GetNamedNumber(L"schema_version")!=1) throw hresult_error(E_INVALIDARG,L"Ismeretlen témaséma.");
    auto kinds=root.GetNamedArray(L"syntax_kind_roles");
    if(kinds.Size()!=syntaxRoles.size()) throw hresult_error(E_INVALIDARG,L"Eltérő szintaxis-témaszerepek.");
    for(uint32_t index=0;index<kinds.Size();++index) if(kinds.GetStringAt(index)!=syntaxRoles[index]) throw hresult_error(E_INVALIDARG,L"Eltérő szintaxis-témasorrend.");
    for(auto const& item:root.GetNamedArray(L"palettes")) {
        auto value=item.GetObject(); EditorPalette palette;
        palette.id=value.GetNamedString(L"id"); palette.name=value.GetNamedString(L"name");
        palette.dark=colors(value.GetNamedObject(L"dark")); palette.light=colors(value.GetNamedObject(L"light")); palettes.push_back(std::move(palette));
    }
    if(palettes.empty()) throw hresult_error(E_INVALIDARG,L"Üres helyi témakatalógus.");
}
EditorPalette const& ThemeCatalog::Palette(hstring const& id) const {
    for(auto const& palette:palettes) if(palette.id==id) return palette;
    return palettes.front();
}
AppearancePreference ThemeCatalog::ReadPreference() const {
    AppearancePreference result;
    try {
        auto path=preferencePath(); if(!std::filesystem::exists(path)) return result;
        auto value=JsonObject::Parse(to_hstring(readFile(path)));
        result.palette=Palette(value.GetNamedString(L"palette",L"graphite")).id;
        auto mode=value.GetNamedString(L"mode",L"system");
        if(mode==L"light" || mode==L"dark") result.mode=mode;
    } catch(...) { /* A broken preference never prevents opening source files. */ }
    return result;
}
void ThemeCatalog::WritePreference(AppearancePreference const& preference) const {
    auto path=preferencePath(); std::filesystem::create_directories(path.parent_path());
    JsonObject value; value.SetNamedValue(L"palette",JsonValue::CreateStringValue(preference.palette)); value.SetNamedValue(L"mode",JsonValue::CreateStringValue(preference.mode));
    std::ofstream stream(path,std::ios::binary|std::ios::trunc); auto text=to_string(value.Stringify());
    if(!stream || !(stream<<text)) throw hresult_error(E_FAIL,L"A helyi megjelenítési beállítás nem menthető.");
}
