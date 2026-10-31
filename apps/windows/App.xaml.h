#pragma once
#include "App.g.h"
namespace winrt::LaTeXIDE::implementation {
struct App : AppT<App> {
    App();
    void OnLaunched(Microsoft::UI::Xaml::LaunchActivatedEventArgs const&);
    Microsoft::UI::Xaml::Window window{nullptr};
};
}
namespace winrt::LaTeXIDE::factory_implementation { struct App : AppT<App, implementation::App> {}; }
