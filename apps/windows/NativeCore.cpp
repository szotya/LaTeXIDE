#include "NativeCore.h"
using namespace winrt;
using namespace Windows::Data::Json;
NativeCore::NativeCore(hstring const& root) {
    auto utf8 = to_string(root);
    handle = latexide_runtime_new(utf8.c_str());
    if (!handle) throw hresult_error(E_FAIL, L"A projekt vagy a helyi session nem nyitható meg.");
}
NativeCore::~NativeCore() { if (handle) latexide_runtime_free(handle); }
JsonObject NativeCore::Call(hstring const& op, JsonObject args) {
    args.SetNamedValue(L"op", JsonValue::CreateStringValue(op));
    auto request = to_string(args.Stringify());
    auto buffer = latexide_dispatch(handle, request.c_str());
    if (!buffer) throw hresult_error(E_FAIL, L"Üres runtime-válasz.");
    std::unique_ptr<char, decltype(&latexide_string_free)> owned(buffer, latexide_string_free);
    auto reply = JsonObject::Parse(to_hstring(buffer));
    if (!reply.GetNamedBoolean(L"ok")) {
        auto error=reply.GetNamedObject(L"error");
        throw NativeCoreError(error.GetNamedString(L"code"),error.GetNamedString(L"message"));
    }
    return reply.GetNamedObject(L"result");
}
NativeSnapshot NativeCore::Read(hstring const& id) {
    NativeSnapshot snapshot{};
    uint64_t offset = 0;
    do {
        JsonObject args; args.SetNamedValue(L"document_id", JsonValue::CreateStringValue(id));
        args.SetNamedValue(L"offset_bytes", JsonValue::CreateNumberValue(static_cast<double>(offset)));
        if (snapshot.revision) args.SetNamedValue(L"revision", JsonValue::CreateNumberValue(static_cast<double>(snapshot.revision)));
        auto part = Call(L"read_document", args);
        snapshot.text += part.GetNamedString(L"text");
        snapshot.revision = static_cast<uint64_t>(part.GetNamedNumber(L"revision"));
        snapshot.dirty = part.GetNamedBoolean(L"dirty");
        snapshot.bom = part.GetNamedBoolean(L"bom");
        snapshot.lineEnding = part.GetNamedString(L"line_ending");
        if (part.GetNamedBoolean(L"done")) break;
        offset = static_cast<uint64_t>(part.GetNamedNumber(L"next_offset"));
    } while (true);
    return snapshot;
}
