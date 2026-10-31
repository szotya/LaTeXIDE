#pragma once
#include "MainWindow.g.h"
#include "NativeCore.h"
#include "ThemeCatalog.h"
#include "TextAdapter.h"
#include <functional>
namespace winrt::LaTeXIDE::implementation {
struct RetainedDocument {
    hstring id;
    bool visible{true}, dirty{}, hasView{};
    int32_t selectionStart{}, selectionEnd{};
    double horizontal{}, vertical{};
};
struct MainWindow : MainWindowT<MainWindow> {
    MainWindow();
    Windows::Foundation::IAsyncAction OpenFolder();
    fire_and_forget OpenProject(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void Save(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void Undo(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void Redo(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void Build(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void Cancel(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void CopyMCP(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void ProjectItemInvoked(Microsoft::UI::Xaml::Controls::TreeView const&, Microsoft::UI::Xaml::Controls::TreeViewItemInvokedEventArgs const&);
    void FilterChanged(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::Controls::TextChangedEventArgs const&);
    void FilterKeyDown(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::Input::KeyRoutedEventArgs const&);
    void TabSelected(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const&);
    void HideTab(Microsoft::UI::Xaml::Controls::TabView const&, Microsoft::UI::Xaml::Controls::TabViewTabCloseRequestedEventArgs const&);
    void AppearanceChanged(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const&);
    void QuickOpen(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void ShowProject(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void ShowAgent(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void ToggleSidebar(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void TogglePreview(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void ToggleOutput(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void ShowDiagnostics(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void ShowCompilerOutput(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void ResizeSidebar(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::Controls::Primitives::DragDeltaEventArgs const&);
    void ResizePreview(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::Controls::Primitives::DragDeltaEventArgs const&);
    void ResizeOutput(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::Controls::Primitives::DragDeltaEventArgs const&);
    void TextChanged(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void CompositionStarted(Microsoft::UI::Xaml::Controls::RichEditBox const&, Microsoft::UI::Xaml::Controls::TextCompositionStartedEventArgs const&);
    void CompositionEnded(Microsoft::UI::Xaml::Controls::RichEditBox const&, Microsoft::UI::Xaml::Controls::TextCompositionEndedEventArgs const&);
    void EditorKeyDown(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::Input::KeyRoutedEventArgs const&);
    void WorkspaceKeyDown(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::Input::KeyRoutedEventArgs const&);
    void EditorSelectionChanged(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void EditorSizeChanged(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::SizeChangedEventArgs const&);
    fire_and_forget PlainTextPaste(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::Controls::TextControlPasteEventArgs const&);
    void RejectRichDrop(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::DragEventArgs const&);
    void ShowFind(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void HideFind(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void FindNext(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void FindPrevious(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void FindKeyDown(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::Input::KeyRoutedEventArgs const&);
    void PreviousPage(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void NextPage(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void ZoomIn(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
    void ZoomOut(Windows::Foundation::IInspectable const&, Microsoft::UI::Xaml::RoutedEventArgs const&);
private:
    std::unique_ptr<NativeCore> core;
    std::unique_ptr<ThemeCatalog> themes;
    AppearancePreference appearance;
    std::u16string mirror;
    std::vector<size_t> lines{0};
    std::vector<hstring> projectFiles;
    std::map<std::wstring, Microsoft::UI::Xaml::Controls::TreeViewNode> fileNodes;
    std::vector<RetainedDocument> retained;
    hstring document, operation, pdfPath;
    uint64_t revision{}, projectGeneration{};
    bool rendering{}, composing{}, compositionRegistered{}, pdfLoading{}, rebuildingTabs{}, changingAppearance{}, themeResetPending{}, closing{};
    bool sidebarVisible{true}, previewVisible{true}, outputVisible{true}, syntaxPending{};
    double sidebarWidth{224}, previewWidth{}, outputHeight{154};
    uint32_t pageIndex{};
    Windows::Data::Pdf::PdfDocument pdf{nullptr};
    Microsoft::UI::Xaml::DispatcherTimer timer{nullptr}, syntaxTimer{nullptr};
    Microsoft::UI::Xaml::Controls::ScrollViewer editorScroll{nullptr};
    void Guard(std::function<void()> action);
    void Load(hstring const& id);
    void Apply(bool composition);
    void History(hstring const& op);
    void SaveAll();
    bool HasDirty();
    void Poll();
    RetainedDocument& Retain(hstring const& id);
    void StoreView();
    void RefreshTabs();
    void HideDocumentTab(hstring const& id);
    void RefreshRetained(Windows::Data::Json::JsonObject const& info);
    void PopulateFiles();
    void ApplyAppearance();
    void ShowSidebar(bool agent);
    void SetOutputVisible(bool visible);
    void InitializeEditor();
    void ScheduleSyntax(bool reset = false);
    void ColorVisibleSyntax();
    source_text::SyntaxWindow VisibleWindow();
    void UpdateEditorAdornments();
    void Find(bool backward);
    Microsoft::UI::Xaml::Media::SolidColorBrush Brush(wchar_t const* key);
    Windows::UI::Color SyntaxColor(uint32_t kind) const;
    fire_and_forget LoadPDF(hstring path);
    fire_and_forget RenderPage();
};
}
namespace winrt::LaTeXIDE::factory_implementation { struct MainWindow : MainWindowT<MainWindow, implementation::MainWindow> {}; }
