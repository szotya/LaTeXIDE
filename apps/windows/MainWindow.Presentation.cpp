#include "pch.h"
#include "MainWindow.xaml.h"
#include "WinUIHelpers.h"
#include <cwctype>
#include <array>
using namespace winrt;
using namespace Windows::Foundation;
using namespace Windows::Data::Json;
using namespace Microsoft::UI::Xaml;
using namespace Microsoft::UI::Xaml::Controls;
using namespace Microsoft::UI::Xaml::Media;
using namespace native_ui;
namespace {
std::wstring lowercase(std::wstring value) { std::transform(value.begin(),value.end(),value.begin(),[](wchar_t c){ return static_cast<wchar_t>(std::towlower(c)); }); return value; }
GridLength pixels(double value) { return {value,GridUnitType::Pixel}; }
GridLength star(double value=1) { return {value,GridUnitType::Star}; }
}
namespace winrt::LaTeXIDE::implementation {
SolidColorBrush MainWindow::Brush(wchar_t const* key) { return Root().Resources().Lookup(box_value(hstring(key))).as<SolidColorBrush>(); }
Windows::UI::Color MainWindow::SyntaxColor(uint32_t kind) const {
    static constexpr std::array<wchar_t const*,11> roles{L"text",L"command",L"environment",L"delimiter",L"comment",L"math",L"parameter",L"literal",L"number",L"operator",L"attribute"};
    auto const& palette=themes->Palette(appearance.palette);
    auto const& colors=Root().ActualTheme()==ElementTheme::Dark?palette.dark:palette.light;
    return colors.at(roles.at(kind));
}
void MainWindow::ApplyAppearance() {
    if(!themes || changingAppearance) return;
    auto requested=appearance.mode==L"light"?ElementTheme::Light:appearance.mode==L"dark"?ElementTheme::Dark:ElementTheme::Default;
    // Default releases a fixed native override. ActualThemeChanged follows the OS.
    if(Root().RequestedTheme()!=requested) Root().RequestedTheme(requested);
    auto const& palette=themes->Palette(appearance.palette);
    auto const& colors=Root().ActualTheme()==ElementTheme::Dark?palette.dark:palette.light;
    constexpr std::array<std::pair<wchar_t const*,wchar_t const*>,14> brushes{{
        {L"IdeChromeBrush",L"chrome"},{L"IdeSidebarBrush",L"sidebar"},{L"IdeEditorBrush",L"editor"},{L"IdePanelBrush",L"panel"},
        {L"IdeBorderBrush",L"border"},{L"IdeSelectionBrush",L"selection"},{L"IdeCurrentLineBrush",L"currentLine"},{L"IdeHoverBrush",L"hover"},
        {L"IdeTextBrush",L"text"},{L"IdeSecondaryBrush",L"secondary"},{L"IdeAccentBrush",L"accent"},{L"IdeSuccessBrush",L"success"},{L"IdeWarningBrush",L"warning"},{L"IdeErrorBrush",L"error"}
    }};
    // Mutate existing brush objects: already-bound XAML and flyouts update too.
    for(auto const& [key,role]:brushes) Brush(key).Color(colors.at(role));
    if(Microsoft::UI::Windowing::AppWindowTitleBar::IsCustomizationSupported()) {
        auto titlebar=AppWindow().TitleBar();
        titlebar.BackgroundColor(colors.at(L"chrome")); titlebar.ForegroundColor(colors.at(L"text"));
        titlebar.InactiveBackgroundColor(colors.at(L"chrome")); titlebar.InactiveForegroundColor(colors.at(L"secondary"));
        titlebar.ButtonBackgroundColor(colors.at(L"chrome")); titlebar.ButtonForegroundColor(colors.at(L"text"));
        titlebar.ButtonHoverBackgroundColor(colors.at(L"hover")); titlebar.ButtonHoverForegroundColor(colors.at(L"text"));
        titlebar.ButtonInactiveBackgroundColor(colors.at(L"chrome")); titlebar.ButtonInactiveForegroundColor(colors.at(L"secondary"));
    }
    Editor().SelectionHighlightColor(Brush(L"IdeSelectionBrush"));
    if(!composing) {
        FlagGuard lock(rendering);
        auto format=Editor().Document().GetDefaultCharacterFormat(); format.ForegroundColor(colors.at(L"text"));
        format.BackgroundColor(colors.at(L"editor")); Editor().Document().SetDefaultCharacterFormat(format);
    }
    ScheduleSyntax(true); UpdateEditorAdornments();
}
void MainWindow::AppearanceChanged(IInspectable const&,SelectionChangedEventArgs const&) {
    if(changingAppearance || !themes || !PaletteChoice().SelectedItem() || !ModeChoice().SelectedItem()) return;
    Guard([this] {
        appearance.palette=unbox_value<hstring>(PaletteChoice().SelectedItem().as<ComboBoxItem>().Tag());
        appearance.mode=unbox_value<hstring>(ModeChoice().SelectedItem().as<ComboBoxItem>().Tag());
        ApplyAppearance(); themes->WritePreference(appearance);
    });
}
RetainedDocument& MainWindow::Retain(hstring const& id) {
    for(auto& item:retained) if(item.id==id) return item;
    // Rust enforces the ten-document budget. Hiding only changes native visibility.
    retained.push_back(RetainedDocument{id}); return retained.back();
}
void MainWindow::StoreView() {
    if(document.empty()) return;
    auto& value=Retain(document); auto selection=Editor().Document().Selection();
    value.selectionStart=selection.StartPosition(); value.selectionEnd=selection.EndPosition(); value.hasView=true;
    if(editorScroll) { value.horizontal=editorScroll.HorizontalOffset(); value.vertical=editorScroll.VerticalOffset(); }
}
void MainWindow::RefreshTabs() {
    if(rebuildingTabs || composing) return;
    FlagGuard lock(rebuildingTabs); DocumentTabs().TabItems().Clear(); uint32_t hidden=0;
    for(auto const& value:retained) {
        if(!value.visible) { ++hidden; continue; }
        TabViewItem tab; tab.Tag(box_value(value.id)); tab.IsClosable(false);
        StackPanel header; header.Orientation(Orientation::Horizontal); header.Spacing(8);
        TextBlock label; label.Text((value.dirty?hstring(L"● "):hstring{})+hstring(std::filesystem::path(value.id.c_str()).filename().native())); label.FontSize(12); label.VerticalAlignment(VerticalAlignment::Center); label.Foreground(Brush(value.id==document?L"IdeTextBrush":L"IdeSecondaryBrush"));
        Button hide; hide.Content(box_value(L"×")); hide.MinWidth(20); hide.MinHeight(20); hide.Padding(Thickness{2}); hide.FontSize(12);
        ToolTipService::SetToolTip(hide,box_value(L"Fül elrejtése")); Automation::AutomationProperties::SetName(hide,L"Fül elrejtése");
        hide.Click([weak=get_weak(),id=value.id](auto const&,RoutedEventArgs const& args){ args.Handled(true); if(auto self=weak.get()) self->HideDocumentTab(id); });
        header.Children().Append(label); header.Children().Append(hide); tab.Header(header);
        tab.Background(Brush(value.id==document?L"IdeEditorBrush":L"IdePanelBrush")); tab.Foreground(Brush(value.id==document?L"IdeTextBrush":L"IdeSecondaryBrush"));
        ToolTipService::SetToolTip(tab,box_value(value.id+L"\nA × elrejti a fület; a szöveg és az előzmény megmarad."));
        DocumentTabs().TabItems().Append(tab); if(value.id==document) DocumentTabs().SelectedItem(tab);
    }
    RetainedLabel().Text(to_hstring(retained.size())+L" / 10 megőrzött"+(hidden?L" · "+to_hstring(hidden)+L" rejtett · Ctrl+P":L""));
}
void MainWindow::RefreshRetained(JsonObject const& info) {
    bool changed=false;
    for(auto const& item:info.GetNamedArray(L"open_documents")) {
        auto value=item.GetObject(); auto id=value.GetNamedString(L"id");
        auto existing=std::find_if(retained.begin(),retained.end(),[&](auto const& view){ return view.id==id; });
        bool dirty=value.GetNamedBoolean(L"dirty");
        if(existing==retained.end()) { Retain(id).dirty=dirty; changed=true; }
        else if(existing->dirty!=dirty) { existing->dirty=dirty; changed=true; }
    }
    // An agent save changes dirty metadata without changing the text revision.
    if(changed) RefreshTabs();
}
void MainWindow::TabSelected(IInspectable const&,SelectionChangedEventArgs const&) {
    if(rebuildingTabs || !DocumentTabs().SelectedItem()) return;
    Guard([this]{
        auto id=unbox_value<hstring>(DocumentTabs().SelectedItem().as<TabViewItem>().Tag());
        if(composing) {
            FlagGuard lock(rebuildingTabs);
            for(auto const& item:DocumentTabs().TabItems()) if(unbox_value<hstring>(item.as<TabViewItem>().Tag())==document) DocumentTabs().SelectedItem(item);
            Status().Text(L"Előbb véglegesítsd a szövegbevitelt."); return;
        }
        if(id!=document) { Load(id); Editor().Focus(FocusState::Programmatic); }
    });
}
void MainWindow::HideTab(TabView const&,TabViewTabCloseRequestedEventArgs const& args) {
    HideDocumentTab(unbox_value<hstring>(args.Tab().Tag()));
}
void MainWindow::HideDocumentTab(hstring const& id) {
    Guard([this,&id]{
        if(composing) { Status().Text(L"Aktív szövegbevitel alatt a fül nem rejthető el."); return; }
        StoreView(); Retain(id).visible=false;
        if(id==document) {
            auto next=std::find_if(retained.begin(),retained.end(),[](auto const& value){ return value.visible; });
            if(next!=retained.end()) Load(next->id);
            else {
                document=L""; revision=0; mirror.clear(); lines={0};
                FlagGuard lock(rendering); Editor().Document().SetText(Microsoft::UI::Text::TextSetOptions::None,L"");
                Editor().IsReadOnly(true); EmptyState().Visibility(Visibility::Visible); Breadcrumb().Text(L"Megőrzött fájl visszanyitása: Ctrl+P");
                CaretLabel().Text(L"—"); EOLLabel().Text(L"—"); Gutter().Children().Clear(); CurrentLine().Visibility(Visibility::Collapsed);
            }
        }
        RefreshTabs(); Status().Text(L"Fül elrejtve. Szövege és előzménye megmarad; Ctrl+P visszanyitja.");
    });
}
void MainWindow::PopulateFiles() {
    if(rendering) return;
    FlagGuard lock(rendering); Files().RootNodes().Clear(); fileNodes.clear();
    auto filter=lowercase(std::wstring(FilenameFilter().Text())); std::map<std::wstring,TreeViewNode> directories;
    auto sorted=projectFiles; std::sort(sorted.begin(),sorted.end());
    for(auto const& file:sorted) {
        std::wstring path(file); std::replace(path.begin(),path.end(),L'\\',L'/');
        if(!filter.empty() && lowercase(path).find(filter)==std::wstring::npos) continue;
        TreeViewNode parent{nullptr}; size_t start=0;
        for(;;) {
            auto slash=path.find(L'/',start); auto folder=slash!=std::wstring::npos; auto key=folder?path.substr(0,slash):path;
            if(folder && directories.contains(key)) { parent=directories.at(key); start=slash+1; continue; }
            auto label=folder?path.substr(start,slash-start):path.substr(start); TreeViewNode node; StackPanel row;
            row.Orientation(Orientation::Horizontal); row.Spacing(7); row.Tag(box_value(folder?hstring{}:file));
            FontIcon icon; icon.FontFamily(FontFamily(L"Segoe Fluent Icons")); icon.FontSize(12); icon.Glyph(folder?L"\xE8B7":L"\xE8A5");
            auto extension=lowercase(std::filesystem::path(path).extension().native());
            icon.Foreground(Brush(folder?L"IdeSecondaryBrush":extension==L".tex"?L"IdeAccentBrush":extension==L".bib"?L"IdeSuccessBrush":L"IdeWarningBrush"));
            TextBlock title; title.Text(label); title.FontSize(12); title.Foreground(Brush(L"IdeTextBrush")); title.TextTrimming(TextTrimming::CharacterEllipsis);
            row.Children().Append(icon); row.Children().Append(title); ToolTipService::SetToolTip(row,box_value(file)); node.Content(row); node.IsExpanded(true);
            if(parent) parent.Children().Append(node); else Files().RootNodes().Append(node);
            if(!folder) { fileNodes.emplace(std::wstring(file),node); break; }
            directories.emplace(key,node); parent=node; start=slash+1;
        }
    }
    if(auto selected=fileNodes.find(std::wstring(document)); selected!=fileNodes.end()) Files().SelectedNode(selected->second);
}
void MainWindow::ProjectItemInvoked(TreeView const&,TreeViewItemInvokedEventArgs const& args) {
    if(rendering) return;
    Guard([this,&args]{
        auto invoked=args.InvokedItem();
        auto node=invoked.try_as<TreeViewNode>(); auto row=(node?node.Content():invoked).try_as<FrameworkElement>(); if(!row || !row.Tag()) return;
        auto id=unbox_value<hstring>(row.Tag()); if(id.empty()) return;
        if(composing) throw hresult_error(E_FAIL,L"Előbb véglegesítsd a szövegbevitelt.");
        Load(id); Editor().Focus(FocusState::Programmatic);
    });
}
void MainWindow::FilterChanged(IInspectable const&,TextChangedEventArgs const&) { Guard([this]{ PopulateFiles(); }); }
void MainWindow::FilterKeyDown(IInspectable const&,Microsoft::UI::Xaml::Input::KeyRoutedEventArgs const& args) {
    using Windows::System::VirtualKey;
    if(args.Key()==VirtualKey::Enter) {
        args.Handled(true); Guard([this]{
            if(composing) return;
            auto filter=lowercase(std::wstring(FilenameFilter().Text()));
            auto first=std::find_if(projectFiles.begin(),projectFiles.end(),[&](auto const& file){ return filter.empty()?std::filesystem::path(file.c_str()).extension()==L".tex":lowercase(std::wstring(file)).find(filter)!=std::wstring::npos; });
            if(first!=projectFiles.end()) { Load(*first); Editor().Focus(FocusState::Programmatic); }
            else Status().Text(L"Nincs ilyen nevű projektfájl.");
        });
    } else if(args.Key()==VirtualKey::Down) { args.Handled(true); Files().Focus(FocusState::Programmatic); }
    else if(args.Key()==VirtualKey::Escape) { args.Handled(true); FilenameFilter().Text(L""); Editor().Focus(FocusState::Programmatic); }
}
void MainWindow::ShowSidebar(bool agent) {
    sidebarVisible=true; Sidebar().Visibility(Visibility::Visible); SidebarDivider().Visibility(Visibility::Visible);
    SidebarColumn().Width(pixels(sidebarWidth)); SidebarDividerColumn().Width(pixels(4));
    AgentPane().Visibility(agent?Visibility::Visible:Visibility::Collapsed); ProjectPane().Visibility(agent?Visibility::Collapsed:Visibility::Visible);
    SidebarTitle().Text(agent?L"HELYI AGENT":L"PROJEKT");
    ProjectRail().Foreground(Brush(agent?L"IdeSecondaryBrush":L"IdeAccentBrush")); AgentRail().Foreground(Brush(agent?L"IdeAccentBrush":L"IdeSecondaryBrush"));
}
void MainWindow::ShowProject(IInspectable const&,RoutedEventArgs const&) { ShowSidebar(false); }
void MainWindow::ShowAgent(IInspectable const&,RoutedEventArgs const&) { ShowSidebar(true); }
void MainWindow::QuickOpen(IInspectable const&,RoutedEventArgs const&) { ShowSidebar(false); FilenameFilter().Focus(FocusState::Programmatic); FilenameFilter().SelectAll(); }
void MainWindow::ToggleSidebar(IInspectable const&,RoutedEventArgs const&) {
    if(!sidebarVisible) { ShowSidebar(false); return; }
    sidebarVisible=false; Sidebar().Visibility(Visibility::Collapsed); SidebarDivider().Visibility(Visibility::Collapsed);
    SidebarColumn().Width(pixels(0)); SidebarDividerColumn().Width(pixels(0));
}
void MainWindow::TogglePreview(IInspectable const&,RoutedEventArgs const&) {
    previewVisible=!previewVisible;
    PDFPane().Visibility(previewVisible?Visibility::Visible:Visibility::Collapsed); PreviewDivider().Visibility(previewVisible?Visibility::Visible:Visibility::Collapsed);
    PreviewDividerColumn().Width(pixels(previewVisible?4:0)); PreviewColumn().Width(previewVisible?(previewWidth?pixels(previewWidth):star(0.78)):pixels(0)); ScheduleSyntax();
}
void MainWindow::SetOutputVisible(bool visible) {
    outputVisible=visible; OutputPane().Visibility(visible?Visibility::Visible:Visibility::Collapsed); OutputDivider().Visibility(visible?Visibility::Visible:Visibility::Collapsed);
    OutputRow().Height(pixels(visible?outputHeight:0)); OutputDividerRow().Height(pixels(visible?4:0)); ScheduleSyntax();
}
void MainWindow::ToggleOutput(IInspectable const&,RoutedEventArgs const&) { SetOutputVisible(!outputVisible); }
void MainWindow::ShowDiagnostics(IInspectable const&,RoutedEventArgs const&) { DiagnosticsScroll().Visibility(Visibility::Visible); LogScroll().Visibility(Visibility::Collapsed); DiagnosticsButton().Foreground(Brush(L"IdeAccentBrush")); CompilerOutputButton().Foreground(Brush(L"IdeSecondaryBrush")); }
void MainWindow::ShowCompilerOutput(IInspectable const&,RoutedEventArgs const&) { DiagnosticsScroll().Visibility(Visibility::Collapsed); LogScroll().Visibility(Visibility::Visible); DiagnosticsButton().Foreground(Brush(L"IdeSecondaryBrush")); CompilerOutputButton().Foreground(Brush(L"IdeAccentBrush")); }
void MainWindow::ResizeSidebar(IInspectable const&,Primitives::DragDeltaEventArgs const& args) { if(sidebarVisible) { sidebarWidth=std::clamp(sidebarWidth+args.HorizontalChange(),160.0,std::max(160.0,Workspace().ActualWidth()*0.4)); SidebarColumn().Width(pixels(sidebarWidth)); } }
void MainWindow::ResizePreview(IInspectable const&,Primitives::DragDeltaEventArgs const& args) { if(previewVisible) { previewWidth=std::clamp(PDFPane().ActualWidth()-args.HorizontalChange(),180.0,std::max(180.0,DocumentWorkspace().ActualWidth()-260)); PreviewColumn().Width(pixels(previewWidth)); } }
void MainWindow::ResizeOutput(IInspectable const&,Primitives::DragDeltaEventArgs const& args) { if(outputVisible) { outputHeight=std::clamp(outputHeight-args.VerticalChange(),90.0,std::max(90.0,WorkArea().ActualHeight()-160)); OutputRow().Height(pixels(outputHeight)); } }
}
