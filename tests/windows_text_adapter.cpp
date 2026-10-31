#include "../apps/windows/TextAdapter.h"
#include <cassert>
#include <iostream>
int main() {
    using namespace source_text;
    auto delta=difference(u"a😀z",u"a😃z");
    assert(delta.start==1 && delta.count==2 && delta.text==u"😃");
    auto column=position(u"első\n😀e\u0301 x",9);
    assert(column.line==1 && column.character==4);
    bool rejected=false;try{position(u"😀",1);}catch(std::out_of_range const&){rejected=true;}assert(rejected);
    assert(fromRichEdit(u"a\rb\r\r")==u"a\nb\n");
    assert(fromRichEdit(u"\r").empty());
    assert(fromRichEdit(u"a\r\nb\r")==u"a\nb");
    std::cout<<"PASS: portable UTF-16 delta, surrogate rejection and paragraph normalization; not a WinUI runtime test\n";
}
