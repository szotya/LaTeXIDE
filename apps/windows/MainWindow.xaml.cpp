#include "pch.h"
#include "MainWindow.xaml.h"
#include "MainWindow.g.i.cpp"
#include "WinUIHelpers.h"
#include "ProjectFiles.h"
using namespace winrt;
using namespace Windows::Foundation;
using namespace Windows::Data::Json;
using namespace Microsoft::UI::Xaml;
using namespace Microsoft::UI::Xaml::Controls;
using namespace native_ui;
namespace {
JsonObject position(source_text::Position const& value) {
    JsonObject object; object.SetNamedValue(L"line",num(value.line)); object.SetNamedValue(L"character",num(value.character)); return object;
}
}
namespace winrt::LaTeXIDE::implementation {
MainWindow::MainWindow() {
    InitializeComponent();
    InitializeEditor();
    Guard([this] {
        themes=std::make_unique<ThemeCatalog>(executableFolder()/L"editor-themes.json");
        appearance=themes->ReadPreference();
        {
            FlagGuard lock(changingAppearance);
            for(auto const& palette:themes->Palettes()) {
                ComboBoxItem item; item.Content(box_value(palette.name)); item.Tag(box_value(palette.id)); PaletteChoice().Items().Append(item);
                if(palette.id==appearance.palette) PaletteChoice().SelectedItem(item);
            }
            ModeChoice().SelectedIndex(appearance.mode==L"light"?1:appearance.mode==L"dark"?2:0);
        }
        ApplyAppearance();
    });
    if(!themes) { PaletteChoice().IsEnabled(false); ModeChoice().IsEnabled(false); }
    Root().ActualThemeChanged([this](auto const&,auto const&) { if(!closing) Guard([this]{ ApplyAppearance(); }); });
    timer=DispatcherTimer(); timer.Interval(std::chrono::milliseconds(300));
    timer.Tick([this](auto const&,auto const&){ if(!closing) Guard([this]{ Poll(); }); }); timer.Start();
    // The native title bar remains responsible for window drag/resize and caption buttons.
    AppWindow().Resize(Windows::Graphics::SizeInt32{1440,940});
    AppWindow().Closing([this](auto const&,Microsoft::UI::Windowing::AppWindowClosingEventArgs const& args) {
        if(composing || HasDirty()) { args.Cancel(true); Status().Text(L"Kilépés előtt véglegesítsd a szövegbevitelt és mentsd a módosításokat."); }
        else { closing=true; ++projectGeneration; timer.Stop(); syntaxTimer.Stop(); core.reset(); }
    });
}
void MainWindow::Guard(std::function<void()> action) {
    try { action(); }
    catch(hresult_error const& error) { Status().Text(error.message()); }
    catch(std::exception const& error) { Status().Text(to_hstring(error.what())); }
}
bool MainWindow::HasDirty() {
    if(!core) return false;
    try { for(auto const& item:core->Call(L"workspace_info").GetNamedArray(L"open_documents")) if(item.GetObject().GetNamedBoolean(L"dirty")) return true; }
    catch(...) { return true; }
    return false;
}
fire_and_forget MainWindow::OpenProject(IInspectable const&,RoutedEventArgs const&) {
    auto lifetime=get_strong();
    try { co_await OpenFolder(); }
    catch(hresult_error const& error) { Status().Text(error.message()); }
    catch(std::exception const& error) { Status().Text(to_hstring(error.what())); }
}
IAsyncAction MainWindow::OpenFolder() {
    if(composing || HasDirty()) { Status().Text(L"Előbb véglegesítsd és mentsd az aktuális projektet."); co_return; }
    Windows::Storage::Pickers::FolderPicker picker; picker.FileTypeFilter().Append(L"*");
    HWND hwnd; check_hresult(this->as<IWindowNative>()->get_WindowHandle(&hwnd));
    check_hresult(picker.as<IInitializeWithWindow>()->Initialize(hwnd));
    auto folder=co_await picker.PickSingleFolderAsync(); if(!folder || closing) co_return;
    if(composing || HasDirty()) { Status().Text(L"Az előző projekt közben módosult; előbb mentsd a módosításokat."); co_return; }
    auto candidate=std::make_unique<NativeCore>(folder.Path());
    auto compiler=executableFolder()/L"texlive"/L"bin"/L"windows"/L"latexmk.exe";
    if(std::filesystem::is_regular_file(compiler)) {
        JsonObject config; config.SetNamedValue(L"compiler_path",str(compiler.native()));
        config.SetNamedValue(L"cache_directory",str((std::filesystem::path(folder.Path().c_str())/L".latexide"/L"texlive-cache").native()));
        config.SetNamedValue(L"output_directory",str((std::filesystem::path(folder.Path().c_str())/L".latexide"/L"builds").native()));
        config.SetNamedValue(L"backend",str(L"texlive")); candidate->Call(L"configure_compiler",config);
    }
    std::vector<hstring> files;
    for(uint64_t offset=0;;) {
        JsonObject args; args.SetNamedValue(L"offset",num(offset)); auto page=candidate->Call(L"list_project_files",args);
        auto entries=page.GetNamedArray(L"files");
        for(auto const& item:entries) files.push_back(item.GetObject().GetNamedString(L"path"));
        offset+=entries.Size(); if(offset>=page.GetNamedNumber(L"total")) break;
        if(!entries.Size()) throw hresult_error(E_FAIL,L"Hiányos fájllista.");
    }
    // An MCP client can edit while the picker/candidate is being prepared.
    if(composing || HasDirty()) { Status().Text(L"Az előző projekt közben módosult; a projektváltás várakozik."); co_return; }
    core=std::move(candidate); ++projectGeneration; retained.clear(); projectFiles=std::move(files);
    document=L""; operation=L""; pdfPath=L""; pdf=nullptr; pdfLoading=false; mirror.clear(); lines={0};
    Preview().Source(nullptr); PageLabel().Text(L"Nincs PDF"); PDFState().Text(L"Még nem fordult");
    { FlagGuard lock(rendering); Editor().Document().SetText(Microsoft::UI::Text::TextSetOptions::None,L""); FilenameFilter().Text(L""); }
    Editor().IsReadOnly(true); EmptyState().Visibility(Visibility::Visible); RefreshTabs(); PopulateFiles();
    auto path=std::filesystem::path(folder.Path().c_str()); ProjectLabel().Text(path.filename().native());
    ToolTipService::SetToolTip(ProjectButton(),box_value(folder.Path()));
    AgentState().Text(L"MCP elérhető"); MCPButton().IsEnabled(true);
    AgentDescription().Text(L"A projekt helyi MCP-munkamenete elérhető. A konfiguráció egy külső, helyben futó agenthez másolható.");
    Log().Text(L"A helyi fordító kimenete itt jelenik meg."); DiagnosticsText().Text(L"Nincs fordítási diagnosztika."); DiagnosticsButton().Content(box_value(L"Problémák · 0"));
    OutputState().Text(L"Nincs fordítási eredmény"); Status().Text(folder.Path());
    std::vector<std::u16string> targetFiles;
    targetFiles.reserve(projectFiles.size());
    for (auto const& file : projectFiles) targetFiles.push_back(units(file));
    auto initial = wide(project_files::initialMainFile(targetFiles));
    MainFile().Text(initial);
    if (std::find(projectFiles.begin(),projectFiles.end(),initial)!=projectFiles.end()) Load(initial);
    Poll();
}
void MainWindow::Load(hstring const& id) {
    if(!core || composing || id.empty()) return;
    StoreView();
    auto snapshot=core->Read(id); auto& view=Retain(id); view.visible=true; view.dirty=snapshot.dirty;
    document=id; revision=snapshot.revision; mirror=units(hstring(snapshot.text)); lines=source_text::lineStarts(mirror);
    {
        FlagGuard lock(rendering);
        Editor().IsReadOnly(true);
        Editor().Document().SetText(Microsoft::UI::Text::TextSetOptions::None,wide(mirror));
        hstring nativeText,rangeText;
        Editor().Document().GetText(Microsoft::UI::Text::TextGetOptions::None,nativeText);
        Editor().Document().GetRange(0,static_cast<int32_t>(mirror.size())).GetText(Microsoft::UI::Text::TextGetOptions::None,rangeText);
        if(source_text::fromRichEdit(units(nativeText))!=mirror || source_text::normalizeParagraphs(units(rangeText))!=mirror)
            throw hresult_error(E_FAIL,L"A natív szerkesztő nem őrizte meg a szöveg pozícióit; a forrás védelméhez a szerkesztés letiltva.");
        auto end=static_cast<int32_t>(mirror.size());
        Editor().Document().Selection().SetRange(std::min(view.selectionStart,end),std::min(view.selectionEnd,end));
    }
    Editor().IsReadOnly(false); EmptyState().Visibility(Visibility::Collapsed);
    EncodingLabel().Text(snapshot.bom?L"UTF-8 BOM":L"UTF-8"); EOLLabel().Text(snapshot.lineEnding==L"crlf"?L"CRLF":L"LF");
    Breadcrumb().Text(ProjectLabel().Text()+L"  ›  "+id); RefreshTabs();
    if(auto selected=fileNodes.find(std::wstring(id)); selected!=fileNodes.end()) Files().SelectedNode(selected->second);
    if(editorScroll && view.hasView) editorScroll.ChangeView(view.horizontal,view.vertical,nullptr,true);
    ScheduleSyntax(true); UpdateEditorAdornments();
}
void MainWindow::Apply(bool composition) {
    if(!core || document.empty()) return;
    hstring rich; Editor().Document().GetText(Microsoft::UI::Text::TextGetOptions::None,rich);
    auto current=source_text::fromRichEdit(units(rich)); auto delta=source_text::difference(mirror,current);
    JsonArray edits;
    if(delta.count || !delta.text.empty()) {
        JsonObject edit; edit.SetNamedValue(L"start",position(source_text::position(mirror,delta.start)));
        edit.SetNamedValue(L"end",position(source_text::position(mirror,delta.start+delta.count)));
        edit.SetNamedValue(L"text",str(wide(delta.text))); edits.Append(edit);
    }
    if(!composition && edits.Size()==0) return;
    auto args=docArgs(document,revision); args.SetNamedValue(L"edits",edits);
    auto changed=core->Call(composition?L"end_composition":L"apply_edits",args);
    revision=static_cast<uint64_t>(changed.GetNamedNumber(L"revision")); mirror=std::move(current);
    if(composition) composing=false;
    auto& active=Retain(document); bool dirty=changed.GetNamedBoolean(L"dirty"); bool badgeChanged=active.dirty!=dirty; active.dirty=dirty;
    lines=source_text::lineStarts(mirror); if(badgeChanged || composition) RefreshTabs(); ScheduleSyntax(); UpdateEditorAdornments();
}
void MainWindow::TextChanged(IInspectable const&,RoutedEventArgs const&) {
    if(!rendering && !composing) Guard([this]{ try { Apply(false); } catch(...) { Load(document); throw; } });
}
void MainWindow::CompositionStarted(RichEditBox const&,TextCompositionStartedEventArgs const&) {
    if(!core || document.empty() || composing) return;
    // Even a rejected begin must protect native marked text until the end event.
    composing=true; compositionRegistered=false; syntaxTimer.Stop();
    Guard([this]{ core->Call(L"begin_composition",docArgs(document,revision)); compositionRegistered=true; });
}
void MainWindow::CompositionEnded(RichEditBox const&,TextCompositionEndedEventArgs const&) {
    if(!composing) return;
    Guard([this]{
        if(!compositionRegistered) {
            composing=false; Load(document);
            Status().Text(L"A forrás a szövegbevitel előtt megváltozott; az aktuális szöveg visszatöltve. Ismételd meg a bevitelt.");
            return;
        }
        compositionRegistered=false;
        try { Apply(true); ScheduleSyntax(); } catch(...) { composing=false; Load(document); throw; }
    });
}
void MainWindow::SaveAll() {
    if(!core || composing) throw hresult_error(E_FAIL,L"Nincs projekt vagy aktív a szövegbevitel.");
    for(auto const& item:core->Call(L"workspace_info").GetNamedArray(L"open_documents")) {
        auto info=item.GetObject(); if(info.GetNamedBoolean(L"dirty")) core->Call(L"save_document",docArgs(info.GetNamedString(L"id"),static_cast<uint64_t>(info.GetNamedNumber(L"revision"))));
    }
    RefreshRetained(core->Call(L"workspace_info"));
}
void MainWindow::Save(IInspectable const&,RoutedEventArgs const&) { Guard([this]{ SaveAll(); Status().Text(L"Minden módosítás mentve"); }); }
void MainWindow::History(hstring const& op) { if(!core || composing) return; auto changed=core->Call(op); Load(changed.GetNamedString(L"id")); RefreshRetained(core->Call(L"workspace_info")); }
void MainWindow::Undo(IInspectable const&,RoutedEventArgs const&) { Guard([this]{ History(L"undo"); }); }
void MainWindow::Redo(IInspectable const&,RoutedEventArgs const&) { Guard([this]{ History(L"redo"); }); }
void MainWindow::Build(IInspectable const&,RoutedEventArgs const&) {
    Guard([this]{ SaveAll(); JsonObject args; args.SetNamedValue(L"main_file",str(MainFile().Text())); operation=core->Call(L"request_build",args).GetNamedString(L"id"); SetOutputVisible(true); ShowCompilerOutput(nullptr,nullptr); PDFState().Text(pdf?L"Korábbi PDF":L"Fordítás…"); Poll(); });
}
void MainWindow::Cancel(IInspectable const&,RoutedEventArgs const&) {
    Guard([this]{ if(!core || operation.empty()) return; JsonObject args; args.SetNamedValue(L"id",str(operation)); core->Call(L"cancel_build",args); });
}
void MainWindow::CopyMCP(IInspectable const&,RoutedEventArgs const&) { Guard([this]{
    if(!core) return;
    JsonObject config,servers,server; server.SetNamedValue(L"command",str((executableFolder()/L"latexide-mcp.exe").native()));
    JsonArray args; args.Append(str(L"--session")); args.Append(str(core->Call(L"workspace_info").GetNamedString(L"session_descriptor")));
    server.SetNamedValue(L"args",args); servers.SetNamedValue(L"latexide",server); config.SetNamedValue(L"mcpServers",servers);
    Windows::ApplicationModel::DataTransfer::DataPackage data; data.SetText(config.Stringify()); Windows::ApplicationModel::DataTransfer::Clipboard::SetContent(data);
    Status().Text(L"MCP-konfiguráció a vágólapon; csak ehhez a nyitott munkamenethez.");
}); }
void MainWindow::Poll() {
    if(!core) return;
    auto info=core->Call(L"workspace_info"); RefreshRetained(info);
    if(!composing && document.empty()) {
        auto first=std::find_if(retained.begin(),retained.end(),[](auto const& value){ return value.visible; });
        if(first!=retained.end()) Load(first->id);
    }
    BuildButton().IsEnabled(info.GetNamedBoolean(L"compiler_ready") && !composing);
    if(!composing && !document.empty()) for(auto const& item:info.GetNamedArray(L"open_documents")) {
        auto value=item.GetObject(); if(value.GetNamedString(L"id")==document && value.GetNamedNumber(L"revision")!=revision) Load(document);
    }
    if(info.GetNamedValue(L"build").ValueType()==JsonValueType::Object) operation=info.GetNamedObject(L"build").GetNamedString(L"id");
    if(operation.empty()) { BuildState().Text(info.GetNamedBoolean(L"compiler_ready")?L"Fordító kész":L"Nincs helyi fordító"); return; }
    JsonObject args; args.SetNamedValue(L"id",str(operation)); auto status=core->Call(L"get_operation",args); auto state=status.GetNamedString(L"state");
    bool active=state==L"queued" || state==L"running";
    CancelButton().IsEnabled(active); BuildButton().IsEnabled(!active && info.GetNamedBoolean(L"compiler_ready") && !composing);
    BuildState().Text(state); BuildState().Foreground(Brush(state==L"succeeded"?L"IdeSuccessBrush":state==L"failed"?L"IdeErrorBrush":L"IdeSecondaryBrush"));
    auto stale=status.GetNamedBoolean(L"stale"); OutputState().Text(state+(stale?L" · elavult forrás":L"")+(state!=L"succeeded" && pdf?L" · korábbi PDF":L""));
    auto log=status.GetNamedString(L"log"); if(Log().Text()!=log) Log().Text(log);
    hstring diagnostics;
    auto values=status.GetNamedArray(L"diagnostics");
    for(auto const& item:values) {
        auto value=item.GetObject(); auto file=value.GetNamedValue(L"file"); auto line=value.GetNamedValue(L"line");
        diagnostics+=value.GetNamedString(L"severity")+L"  ";
        if(file.ValueType()==JsonValueType::String) diagnostics+=file.GetString();
        if(line.ValueType()==JsonValueType::Number) diagnostics+=L":"+to_hstring(static_cast<uint32_t>(line.GetNumber()));
        diagnostics+=L"  "+value.GetNamedString(L"message")+L"\n";
    }
    DiagnosticsButton().Content(box_value(L"Problémák · "+to_hstring(values.Size())));
    auto diagnosticText=diagnostics.empty()?hstring(L"Nincs fordítási diagnosztika."):diagnostics;
    if(DiagnosticsText().Text()!=diagnosticText) DiagnosticsText().Text(diagnosticText);
    PDFState().Text(stale?L"Elavult forrás":state!=L"succeeded" && pdf?L"Korábbi PDF":state==L"succeeded"?L"Sikeres fordítás":L"Még nincs PDF");
    if(state==L"succeeded") { auto path=status.GetNamedString(L"pdf_path"); if(path!=pdfPath && !pdfLoading) LoadPDF(path); }
}
fire_and_forget MainWindow::LoadPDF(hstring path) {
    auto lifetime=get_strong(); auto generation=projectGeneration; pdfLoading=true;
    try {
        auto file=co_await Windows::Storage::StorageFile::GetFileFromPathAsync(path);
        auto loaded=co_await Windows::Data::Pdf::PdfDocument::LoadFromFileAsync(file);
        if(!closing && generation==projectGeneration) { pdf=loaded; pdfPath=path; pageIndex=0; RenderPage(); }
    } catch(hresult_error const& error) { if(!closing && generation==projectGeneration) Status().Text(error.message()); }
    if(generation==projectGeneration) pdfLoading=false;
}
fire_and_forget MainWindow::RenderPage() {
    auto lifetime=get_strong(); if(!pdf || pageIndex>=pdf.PageCount() || closing) co_return;
    auto current=pageIndex; auto expected=pdfPath; auto generation=projectGeneration; auto source=pdf;
    try {
        auto page=source.GetPage(current); Windows::Storage::Streams::InMemoryRandomAccessStream stream;
        Windows::Data::Pdf::PdfPageRenderOptions options; options.DestinationWidth(1600); co_await page.RenderToStreamAsync(stream,options);
        Microsoft::UI::Xaml::Media::Imaging::BitmapImage image; co_await image.SetSourceAsync(stream);
        if(!closing && generation==projectGeneration && current==pageIndex && expected==pdfPath) {
            Preview().Source(image); PageLabel().Text(to_hstring(current+1)+L" / "+to_hstring(source.PageCount()));
        }
        page.Close();
    } catch(hresult_error const& error) { if(!closing && generation==projectGeneration) Status().Text(error.message()); }
}
void MainWindow::PreviousPage(IInspectable const&,RoutedEventArgs const&) { if(pdf && pageIndex) { --pageIndex; RenderPage(); } }
void MainWindow::NextPage(IInspectable const&,RoutedEventArgs const&) { if(pdf && pageIndex+1<pdf.PageCount()) { ++pageIndex; RenderPage(); } }
void MainWindow::ZoomIn(IInspectable const&,RoutedEventArgs const&) { if(pdf) PDFScroll().ChangeView(nullptr,nullptr,std::min(3.0f,PDFScroll().ZoomFactor()*1.2f)); }
void MainWindow::ZoomOut(IInspectable const&,RoutedEventArgs const&) { if(pdf) PDFScroll().ChangeView(nullptr,nullptr,std::max(0.5f,PDFScroll().ZoomFactor()/1.2f)); }
}
