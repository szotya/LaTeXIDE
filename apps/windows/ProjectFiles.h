#pragma once
#include <string>
#include <vector>
#include <algorithm>

namespace project_files {
inline std::u16string initialMainFile(std::vector<std::u16string> const& files) {
    // Start each project from its own files rather than a previous project's target.
    auto main = std::find(files.begin(), files.end(), u"main.tex");
    if (main != files.end()) return *main;
    auto first = std::find_if(files.begin(), files.end(), [](auto const& file) { return file.ends_with(u".tex"); });
    return first == files.end() ? u"main.tex" : *first;
}
}
