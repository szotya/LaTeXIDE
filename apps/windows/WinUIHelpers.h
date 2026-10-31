#pragma once
#include "pch.h"
#include "TextAdapter.h"
namespace native_ui {
struct FlagGuard {
    bool& flag;
    bool previous;
    explicit FlagGuard(bool& value) : flag(value), previous(value) { flag=true; }
    ~FlagGuard() { flag=previous; }
};
inline std::u16string units(winrt::hstring const& text) {
    static_assert(sizeof(wchar_t)==2);
    return {reinterpret_cast<char16_t const*>(text.c_str()), text.size()};
}
inline winrt::hstring wide(std::u16string const& text) {
    return winrt::hstring(reinterpret_cast<wchar_t const*>(text.data()), static_cast<uint32_t>(text.size()));
}
inline winrt::Windows::Data::Json::JsonValue str(winrt::hstring const& value) { return winrt::Windows::Data::Json::JsonValue::CreateStringValue(value); }
inline winrt::Windows::Data::Json::JsonValue num(uint64_t value) { return winrt::Windows::Data::Json::JsonValue::CreateNumberValue(static_cast<double>(value)); }
inline winrt::Windows::Data::Json::JsonObject docArgs(winrt::hstring const& id, uint64_t revision) {
    winrt::Windows::Data::Json::JsonObject args;
    args.SetNamedValue(L"document_id", str(id)); args.SetNamedValue(L"expected_revision", num(revision)); return args;
}
inline std::filesystem::path executableFolder() {
    wchar_t buffer[32768]; auto length=GetModuleFileNameW(nullptr,buffer,32768);
    if(!length || length==32768) throw winrt::hresult_error(E_FAIL, L"Az alkalmazás útvonala nem olvasható.");
    return std::filesystem::path(std::wstring(buffer,length)).parent_path();
}
template<typename T> T descendant(winrt::Microsoft::UI::Xaml::DependencyObject const& root) {
    using winrt::Microsoft::UI::Xaml::Media::VisualTreeHelper;
    if (auto found=root.try_as<T>()) return found;
    for (int index=0; index<VisualTreeHelper::GetChildrenCount(root); ++index)
        if (auto found=descendant<T>(VisualTreeHelper::GetChild(root,index))) return found;
    return nullptr;
}
}
