#pragma once
#include "pch.h"
#include "latexide_core.h"
struct NativeSnapshot { std::wstring text; uint64_t revision; bool dirty; bool bom; winrt::hstring lineEnding; };
// Keep the machine error code so a syntax race can be retried quietly without
// swallowing unrelated failures or disturbing the text/history authority.
struct NativeCoreError : winrt::hresult_error {
    winrt::hstring code;
    NativeCoreError(winrt::hstring const& code, winrt::hstring const& message) : hresult_error(E_FAIL,message), code(code) {}
};
class NativeCore final {
    LatexideRuntime* handle{};
public:
    explicit NativeCore(winrt::hstring const& root);
    ~NativeCore();
    NativeCore(NativeCore const&) = delete;
    NativeCore& operator=(NativeCore const&) = delete;
    winrt::Windows::Data::Json::JsonObject Call(winrt::hstring const& op, winrt::Windows::Data::Json::JsonObject args = {});
    NativeSnapshot Read(winrt::hstring const& id);
};
