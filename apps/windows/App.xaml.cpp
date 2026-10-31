#include "pch.h"
#include "App.xaml.h"
#include "MainWindow.xaml.h"
#include "App.g.i.cpp"
namespace winrt::LaTeXIDE::implementation {
App::App() { InitializeComponent(); }
void App::OnLaunched(Microsoft::UI::Xaml::LaunchActivatedEventArgs const&) {
    window = make<MainWindow>(); window.Activate();
}
}
