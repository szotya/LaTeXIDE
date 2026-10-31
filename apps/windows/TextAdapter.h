#pragma once
#include <string>
#include <algorithm>
#include <stdexcept>
#include <vector>
// Windows wchar_t/WinRT strings are UTF-16. This header is also exercised with
// portable u16string tests on Mac; RichEdit control conventions still need Windows.
namespace source_text {
struct Position { size_t line{}, character{}; };
struct Delta { size_t start{}, count{}; std::u16string text; };
inline bool high(char16_t c) { return c >= 0xD800 && c <= 0xDBFF; }
inline bool low(char16_t c) { return c >= 0xDC00 && c <= 0xDFFF; }
inline bool scalarBoundary(std::u16string const& text, size_t offset) {
    return offset <= text.size() && !(offset && offset < text.size() && high(text[offset-1]) && low(text[offset]));
}
struct SyntaxWindow { size_t offset{}, length{}; };
inline SyntaxWindow syntaxWindow(std::u16string const& text, size_t first, size_t last, size_t cap = 32768) {
    auto start = std::min(first, text.size());
    if (!scalarBoundary(text, start)) --start;
    auto end = std::min(text.size(), start + std::min(cap, size_t{32768}));
    end = std::min(end, std::max(start, last));
    if (!scalarBoundary(text, end)) --end;
    return {start, end-start};
}
inline std::vector<size_t> lineStarts(std::u16string const& text) {
    std::vector<size_t> result{0};
    for (size_t index=0; index<text.size(); ++index) if (text[index]==u'\n') result.push_back(index+1);
    return result;
}
inline Delta difference(std::u16string const& before, std::u16string const& after) {
    size_t start = 0, tail = 0;
    while (start < std::min(before.size(), after.size()) && before[start] == after[start]) ++start;
    if (start && ((start < before.size() && low(before[start]) && high(before[start - 1])) ||
                  (start < after.size() && low(after[start]) && high(after[start - 1])))) --start;
    while (tail < std::min(before.size(), after.size()) - start && before[before.size()-1-tail] == after[after.size()-1-tail]) ++tail;
    if (tail && ((before.size() > tail && low(before[before.size()-tail]) && high(before[before.size()-tail-1])) ||
                 (after.size() > tail && low(after[after.size()-tail]) && high(after[after.size()-tail-1])))) --tail;
    return {start, before.size()-start-tail, after.substr(start, after.size()-start-tail)};
}
inline Position position(std::u16string const& text, size_t offset) {
    if (offset > text.size() || (offset && offset < text.size() && high(text[offset-1]) && low(text[offset]))) throw std::out_of_range("UTF-16 boundary");
    Position result;
    for (size_t i=0; i<offset; ++i) { if (text[i] == u'\n') { ++result.line; result.character=0; } else ++result.character; }
    return result;
}
inline std::u16string normalizeParagraphs(std::u16string const& text) {
    std::u16string result;
    for (size_t i=0; i<text.size(); ++i) {
        if (text[i] == u'\r') { result += u'\n'; if (i+1<text.size() && text[i+1] == u'\n') ++i; }
        else result += text[i];
    }
    return result;
}
inline std::u16string fromRichEdit(std::u16string text) {
    // The initial RichEdit adapter expects one implicit final CR. Runtime
    // qualification must prove this and TOM UTF-16 positions on Windows.
    if (!text.empty() && text.back() == u'\r') text.pop_back();
    return normalizeParagraphs(text);
}
}
