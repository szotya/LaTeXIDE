#include "../TextAdapter.h"
#include "../ProjectFiles.h"
#include <cassert>
#include <iostream>
int main() {
    using namespace source_text;
    assert(project_files::initialMainFile({u"references.bib",u"thesis.tex"})==u"thesis.tex");
    assert(project_files::initialMainFile({u"chapter.tex",u"main.tex"})==u"main.tex");
    assert(project_files::initialMainFile({u"readme.md"})==u"main.tex");
    // A viewport starting between an emoji's surrogates must never split it.
    auto clipped = syntaxWindow(u"a😀bc", 2, 4, 3);
    assert(clipped.offset == 1 && clipped.length == 3);
    auto capped = syntaxWindow(u"ab😀z", 0, 5, 3);
    assert(capped.offset == 0 && capped.length == 2);
    auto empty = syntaxWindow(u"", 0, 0);
    assert(empty.offset == 0 && empty.length == 0);
    auto starts = lineStarts(u"első\n😀\n");
    assert((starts == std::vector<size_t>{0,5,8}));
    // Exact TOM text verification fails closed if paragraph positions differ.
    assert(normalizeParagraphs(u"a\rb\r\n") == u"a\nb\n");
    assert(fromRichEdit(u"a\rb\r\r") == u"a\nb\n");
    assert(scalarBoundary(u"a😀b", 1));
    assert(!scalarBoundary(u"a😀b", 2));
    assert(scalarBoundary(u"a😀b", 3));
    auto large=std::u16string(50000,u'a'); large.replace(32767,2,u"😀");
    assert(syntaxWindow(large,0,large.size(),99999).length==32767);
    for(auto const& source : {std::u16string(u"a😀b\n😃"),std::u16string(u"\n\n"),std::u16string{}})
        for(size_t first=0;first<=source.size()+2;++first)
            for(size_t last=0;last<=source.size()+2;++last)
                for(size_t cap=0;cap<9;++cap) {
                    auto result=syntaxWindow(source,first,last,cap);
                    assert(result.offset+result.length<=source.size());
                    assert(result.length<=cap);
                    assert(scalarBoundary(source,result.offset));
                    assert(scalarBoundary(source,result.offset+result.length));
                }
    std::cout << "PASS: scalar-safe bounded syntax windows, logical lines and paragraph normalization (portable functions only)\n";
}
