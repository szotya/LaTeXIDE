#include "pch.h"
#include "MainWindow.xaml.h"
#include "WinUIHelpers.h"
#include <array>
using namespace winrt;
using namespace Windows::Foundation;
using namespace Windows::Data::Json;
using namespace Microsoft::UI::Text;
using namespace Microsoft::UI::Xaml;
using namespace Microsoft::UI::Xaml::Controls;
using namespace Microsoft::UI::Xaml::Input;
using namespace Microsoft::UI::Xaml::Media;
using namespace native_ui;
namespace {
bool keyDown(Windows::System::VirtualKey key) {
    return (Microsoft::UI::Input::InputKeyboardSource::GetKeyStateForCurrentThread(key)&Windows::UI::Core::CoreVirtualKeyStates::Down)!=Windows::UI::Core::CoreVirtualKeyStates::None;
}
void copyText(Microsoft::UI::Text::ITextSelection const& selection) {
    hstring value; selection.GetText(TextGetOptions::None,value);
    Windows::ApplicationModel::DataTransfer::DataPackage data; data.SetText(wide(source_text::normalizeParagraphs(units(value))));
    Windows::ApplicationModel::DataTransfer::Clipboard::SetContent(data);
}
}
namespace winrt::LaTeXIDE::implementation {
void MainWindow::InitializeEditor() {
    Editor().Document().UndoLimit(0);
    auto format=Editor().Document().GetDefaultCharacterFormat(); format.Name(L"Cascadia Mono"); format.Size(13);
    Editor().Document().SetDefaultCharacterFormat(format);
    // No formatting UI or native history can create an alternate text authority.
    Editor().DisabledFormattingAccelerators(Microsoft::UI::Xaml::Controls::DisabledFormattingAccelerators::All);
    Editor().SelectionFlyout(nullptr); MenuFlyout menu;
    auto add=[&](hstring const& label,auto action) { MenuFlyoutItem item; item.Text(label); item.Click(action); menu.Items().Append(item); };
    auto weak=get_weak();
    add(L"Kivágás",[weak](auto const&,auto const&) { if(auto self=weak.get()) self->Guard([&]{ if(self->composing || self->document.empty() || self->Editor().IsReadOnly()) return; auto selection=self->Editor().Document().Selection(); copyText(selection); selection.SetText(TextSetOptions::None,L""); }); });
    add(L"Másolás",[weak](auto const&,auto const&) { if(auto self=weak.get()) self->Guard([&]{ copyText(self->Editor().Document().Selection()); }); });
    add(L"Beillesztés egyszerű szövegként",[weak](auto const&,auto const&) { if(auto self=weak.get()) self->PlainTextPaste(nullptr,nullptr); });
    menu.Items().Append(MenuFlyoutSeparator());
    add(L"Visszavonás",[weak](auto const&,auto const&) { if(auto self=weak.get()) self->Undo(nullptr,nullptr); });
    add(L"Újra",[weak](auto const&,auto const&) { if(auto self=weak.get()) self->Redo(nullptr,nullptr); });
    Editor().ContextFlyout(menu);
    syntaxTimer=DispatcherTimer(); syntaxTimer.Interval(std::chrono::milliseconds(120));
    syntaxTimer.Tick([weak](auto const&,auto const&) {
        if(auto self=weak.get()) { self->syntaxTimer.Stop(); if(!self->closing) self->Guard([&]{ self->ColorVisibleSyntax(); }); }
    });
    Editor().Loaded([weak](auto const&,auto const&) {
        if(auto self=weak.get()) {
            if(!self->editorScroll) {
                self->editorScroll=descendant<ScrollViewer>(self->Editor());
                if(self->editorScroll) self->editorScroll.ViewChanged([weak](auto const&,auto const&) {
                    if(auto current=weak.get()) { if(!current->closing) current->Guard([&]{ current->ScheduleSyntax(); current->UpdateEditorAdornments(); }); }
                });
            }
            self->ScheduleSyntax(); self->UpdateEditorAdornments();
        }
    });
}
void MainWindow::ScheduleSyntax(bool reset) {
    themeResetPending=themeResetPending || reset; syntaxPending=true;
    if(!syntaxTimer || composing || closing) return;
    syntaxTimer.Stop(); syntaxTimer.Start();
}
source_text::SyntaxWindow MainWindow::VisibleWindow() {
    if(mirror.empty()) return {};
    size_t first=static_cast<size_t>(std::max(0,Editor().Document().Selection().StartPosition())); size_t last=first+8192;
    try {
        auto top=Editor().Document().GetRangeFromPoint(Point{8,8},PointOptions::ClientCoordinates);
        auto bottom=Editor().Document().GetRangeFromPoint(Point{static_cast<float>(std::max(8.0,Editor().ActualWidth()-20)),static_cast<float>(std::max(8.0,Editor().ActualHeight()-16))},PointOptions::ClientCoordinates);
        first=static_cast<size_t>(std::max(0,top.StartPosition())); last=static_cast<size_t>(std::max(top.StartPosition(),bottom.EndPosition()));
    } catch(hresult_error const&) { /* Before native layout, color around the caret. */ }
    return source_text::syntaxWindow(mirror,first>1024?first-1024:0,last+2048);
}
void MainWindow::ColorVisibleSyntax() {
    if(!syntaxPending || !themes || !core || document.empty() || composing || rendering || closing) return;
    auto window=VisibleWindow(); if(!window.length) { syntaxPending=false; return; }
    auto expectedDocument=document; auto expectedRevision=revision;
    auto args=docArgs(expectedDocument,expectedRevision); args.SetNamedValue(L"offset_utf16",num(window.offset)); args.SetNamedValue(L"length_utf16",num(window.length));
    JsonObject response;
    try { response=core->Call(L"get_syntax",args); }
    catch(NativeCoreError const& error) {
        if(error.code==L"revision_conflict") { ScheduleSyntax(); return; }
        throw;
    }
    if(composing || expectedDocument!=document || expectedRevision!=revision || response.GetNamedString(L"document_id")!=document || response.GetNamedNumber(L"revision")!=revision || response.GetNamedNumber(L"offset_utf16")!=window.offset || response.GetNamedNumber(L"length_utf16")!=window.length) { ScheduleSyntax(); return; }
    auto native=Editor().Document();
    // TOM uses UTF-16 character positions with paragraph markers. Verify actual
    // range text before attributing it; real WinUI paragraph/CRLF qualification
    // is still required. A mismatch fails closed and never changes source text.
    auto matchingRange=[&](size_t offset,size_t length) {
        auto range=native.GetRange(static_cast<int32_t>(offset),static_cast<int32_t>(offset+length)); hstring text;
        range.GetText(TextGetOptions::None,text);
        return source_text::normalizeParagraphs(units(text))==mirror.substr(offset,length);
    };
    if(!matchingRange(window.offset,window.length)) { syntaxPending=false; return; }
    std::vector<std::array<uint32_t,3>> spans; uint64_t previous=0;
    for(auto const& item:response.GetNamedArray(L"spans")) {
        auto value=item.GetArray(); if(value.Size()!=3) throw hresult_error(E_FAIL,L"Hibás szintaxisválasz.");
        auto startNumber=value.GetNumberAt(0), lengthNumber=value.GetNumberAt(1), kindNumber=value.GetNumberAt(2);
        if(startNumber<0 || lengthNumber<=0 || kindNumber<0 || startNumber>mirror.size() || lengthNumber>mirror.size() || kindNumber>=11) throw hresult_error(E_FAIL,L"Hibás szintaxistartomány.");
        auto start=static_cast<uint32_t>(startNumber), length=static_cast<uint32_t>(lengthNumber), kind=static_cast<uint32_t>(kindNumber);
        uint64_t end=static_cast<uint64_t>(start)+length;
        if(startNumber!=start || lengthNumber!=length || kindNumber!=kind || start<previous || end>mirror.size() || !source_text::scalarBoundary(mirror,start) || !source_text::scalarBoundary(mirror,end)) throw hresult_error(E_FAIL,L"Hibás szintaxishatár.");
        if(!matchingRange(start,length)) { syntaxPending=false; return; }
        spans.push_back({start,length,kind}); previous=end;
    }
    auto selection=native.Selection(); auto typing=selection.CharacterFormat().GetClone();
    bool collapsed=selection.StartPosition()==selection.EndPosition();
    auto horizontal=editorScroll?editorScroll.HorizontalOffset():0; auto vertical=editorScroll?editorScroll.VerticalOffset():0;
    FlagGuard lock(rendering); native.BatchDisplayUpdates();
    try {
        if(themeResetPending) {
            auto defaults=native.GetDefaultCharacterFormat(); defaults.ForegroundColor(SyntaxColor(0)); defaults.Name(L"Cascadia Mono"); defaults.Size(13);
            native.SetDefaultCharacterFormat(defaults);
            native.GetRange(0,static_cast<int32_t>(mirror.size())).CharacterFormat().ForegroundColor(SyntaxColor(0));
            themeResetPending=false;
        }
        native.GetRange(static_cast<int32_t>(window.offset),static_cast<int32_t>(window.offset+window.length)).CharacterFormat().ForegroundColor(SyntaxColor(0));
        for(auto const& span:spans) native.GetRange(static_cast<int32_t>(span[0]),static_cast<int32_t>(span[0]+span[1])).CharacterFormat().ForegroundColor(SyntaxColor(span[2]));
        // Only an insertion-point format can be restored without recoloring a
        // selected range. Range formatting above never changes selection.
        if(collapsed) selection.CharacterFormat(typing);
        native.ApplyDisplayUpdates();
    } catch(...) { native.ApplyDisplayUpdates(); throw; }
    if(editorScroll && (editorScroll.HorizontalOffset()!=horizontal || editorScroll.VerticalOffset()!=vertical)) editorScroll.ChangeView(horizontal,vertical,nullptr,true);
    syntaxPending=false; UpdateEditorAdornments();
}
void MainWindow::UpdateEditorAdornments() {
    if(rendering || composing || closing) return;
    Gutter().Children().Clear(); if(document.empty()) { CurrentLine().Visibility(Visibility::Collapsed); return; }
    auto selection=Editor().Document().Selection(); auto caret=std::min(mirror.size(),static_cast<size_t>(std::max(0,selection.EndPosition())));
    if(!source_text::scalarBoundary(mirror,caret)) --caret;
    auto position=source_text::position(mirror,caret); CaretLabel().Text(to_hstring(position.line+1)+L":"+to_hstring(position.character+1));
    RectangleGeometry clip; clip.Rect(Rect{0,0,48,static_cast<float>(Gutter().ActualHeight())}); Gutter().Clip(clip);
    auto window=VisibleWindow(); auto first=std::upper_bound(lines.begin(),lines.end(),window.offset); if(first!=lines.begin()) --first;
    for(auto current=first; current!=lines.end() && *current<=window.offset+window.length; ++current) {
        Rect rect{}; int hit{};
        try { Editor().Document().GetRange(static_cast<int32_t>(*current),static_cast<int32_t>(*current)).GetRect(PointOptions::ClientCoordinates|PointOptions::Start,rect,hit); }
        catch(hresult_error const&) { continue; }
        if(rect.Height<=0 || rect.Y+rect.Height<0 || rect.Y>Editor().ActualHeight()) continue;
        TextBlock label; label.Text(to_hstring(static_cast<size_t>(current-lines.begin())+1)); label.FontFamily(FontFamily(L"Cascadia Mono")); label.FontSize(11);
        label.Foreground(Brush(static_cast<size_t>(current-lines.begin())==position.line?L"IdeAccentBrush":L"IdeSecondaryBrush")); label.Width(36); label.TextAlignment(TextAlignment::Right);
        Canvas::SetLeft(label,4); Canvas::SetTop(label,rect.Y); Gutter().Children().Append(label);
    }
    Rect line{}; int hit{};
    try {
        auto start=lines.at(position.line); Editor().Document().GetRange(static_cast<int32_t>(start),static_cast<int32_t>(start)).GetRect(PointOptions::ClientCoordinates|PointOptions::Start,line,hit);
        CurrentLine().Width(Editor().ActualWidth()); CurrentLine().Height(std::max(17.0f,line.Height)); Canvas::SetTop(CurrentLine(),line.Y);
        CurrentLine().Visibility(line.Height>0 && line.Y>=0 && line.Y<Editor().ActualHeight()?Visibility::Visible:Visibility::Collapsed);
    } catch(hresult_error const&) { CurrentLine().Visibility(Visibility::Collapsed); }
}
void MainWindow::EditorSelectionChanged(IInspectable const&,RoutedEventArgs const&) { if(!rendering && !composing) Guard([this]{ UpdateEditorAdornments(); }); }
void MainWindow::EditorSizeChanged(IInspectable const&,SizeChangedEventArgs const&) { if(!rendering) Guard([this]{ ScheduleSyntax(); UpdateEditorAdornments(); }); }
void MainWindow::WorkspaceKeyDown(IInspectable const&,KeyRoutedEventArgs const& args) {
    using Windows::System::VirtualKey;
    // AltGr is reported as Ctrl+Alt on international keyboards.
    if(args.Handled() || !keyDown(VirtualKey::Control) || keyDown(VirtualKey::Menu)) return;
    auto key=args.Key(); bool shift=keyDown(VirtualKey::Shift);
    if(key==VirtualKey::P) { args.Handled(true); if(shift) CommandFlyout().ShowAt(CommandButton()); else QuickOpen(nullptr,nullptr); }
    else if(key==VirtualKey::F) { args.Handled(true); ShowFind(nullptr,nullptr); }
    else if(key==VirtualKey::S) { args.Handled(true); Save(nullptr,nullptr); }
    else if(key==VirtualKey::B) { args.Handled(true); Build(nullptr,nullptr); }
    else if(key==VirtualKey::W && !document.empty()) { args.Handled(true); HideDocumentTab(document); }
    else if(key==VirtualKey::Z || key==VirtualKey::Y) { args.Handled(true); Guard([&]{ History(key==VirtualKey::Y || shift?L"redo":L"undo"); }); }
}
void MainWindow::EditorKeyDown(IInspectable const& sender,KeyRoutedEventArgs const& args) {
    WorkspaceKeyDown(sender,args); if(args.Handled()) return;
    using Windows::System::VirtualKey;
    if(args.Key()==VirtualKey::Tab && !keyDown(VirtualKey::Control) && !keyDown(VirtualKey::Shift) && !composing && !document.empty() && !Editor().IsReadOnly()) {
        args.Handled(true); Guard([this]{ Editor().Document().Selection().SetText(TextSetOptions::None,L"    "); }); return;
    }
    // Remaining editing/navigation keys are native. The documented
    // DisabledFormattingAccelerators property suppresses RichEdit formatting.
}
fire_and_forget MainWindow::PlainTextPaste(IInspectable const&,TextControlPasteEventArgs const& args) {
    if(args) args.Handled(true);
    auto lifetime=get_strong();
    if(!core || document.empty() || composing || Editor().IsReadOnly()) co_return;
    auto expected=document; auto expectedRevision=revision; auto generation=projectGeneration;
    auto first=Editor().Document().Selection().StartPosition(), last=Editor().Document().Selection().EndPosition();
    try {
        auto content=Windows::ApplicationModel::DataTransfer::Clipboard::GetContent();
        if(!content.Contains(Windows::ApplicationModel::DataTransfer::StandardDataFormats::Text())) co_return;
        auto text=co_await content.GetTextAsync();
        if(closing || composing || Editor().IsReadOnly() || generation!=projectGeneration || expected!=document || expectedRevision!=revision || Editor().Document().Selection().StartPosition()!=first || Editor().Document().Selection().EndPosition()!=last) co_return;
        // TextSetOptions::None consumes text only; no RTF/HTML or clipboard style.
        Editor().Document().Selection().SetText(TextSetOptions::None,wide(source_text::normalizeParagraphs(units(text))));
    } catch(hresult_error const& error) { if(!closing) Status().Text(error.message()); }
}
void MainWindow::RejectRichDrop(IInspectable const&,DragEventArgs const& args) { args.Handled(true); args.AcceptedOperation(Windows::ApplicationModel::DataTransfer::DataPackageOperation::None); }
void MainWindow::ShowFind(IInspectable const&,RoutedEventArgs const&) { FindBar().Visibility(Visibility::Visible); FindQuery().Focus(FocusState::Programmatic); FindQuery().SelectAll(); }
void MainWindow::HideFind(IInspectable const&,RoutedEventArgs const&) { FindBar().Visibility(Visibility::Collapsed); Editor().Focus(FocusState::Programmatic); }
void MainWindow::Find(bool backward) {
    if(document.empty() || composing) return;
    auto query=units(FindQuery().Text()); if(query.empty()) return;
    auto selection=Editor().Document().Selection(); auto start=static_cast<size_t>(std::max(0,backward?selection.StartPosition():selection.EndPosition()));
    size_t found;
    if(backward) { found=start?mirror.rfind(query,start-1):std::u16string::npos; if(found==std::u16string::npos) found=mirror.rfind(query); }
    else { found=mirror.find(query,start); if(found==std::u16string::npos) found=mirror.find(query); }
    if(found==std::u16string::npos) { Status().Text(L"Nincs találat a forrásban."); return; }
    selection.SetRange(static_cast<int32_t>(found),static_cast<int32_t>(found+query.size())); selection.ScrollIntoView(PointOptions::None);
    Status().Text(L"Találat · a keresésben a kis- és nagybetű számít"); UpdateEditorAdornments();
}
void MainWindow::FindNext(IInspectable const&,RoutedEventArgs const&) { Guard([this]{ Find(false); }); }
void MainWindow::FindPrevious(IInspectable const&,RoutedEventArgs const&) { Guard([this]{ Find(true); }); }
void MainWindow::FindKeyDown(IInspectable const&,KeyRoutedEventArgs const& args) {
    if(args.Key()==Windows::System::VirtualKey::Enter) { args.Handled(true); Guard([this]{ Find(keyDown(Windows::System::VirtualKey::Shift)); }); }
    else if(args.Key()==Windows::System::VirtualKey::Escape) { args.Handled(true); HideFind(nullptr,nullptr); }
}
}
