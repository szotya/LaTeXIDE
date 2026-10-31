import SwiftUI
struct AppearancePicker: View {
    @ObservedObject var preferences: EditorPreferences
    @Environment(\.ideTheme) private var theme
    @Environment(\.colorScheme) private var systemScheme
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack { Text("Megjelenés").font(.system(size: 15, weight: .semibold)); Spacer(); Image(systemName: "paintpalette").foregroundStyle(theme.ui("accent")) }
            HStack(spacing: 4) {
                ForEach(AppearanceMode.allCases) { mode in
                    Button { preferences.appearance = mode } label: {
                        VStack(spacing: 6) { Image(systemName: mode.symbol).font(.system(size: 16)); Text(mode.title).font(.system(size: 11, weight: .medium)) }
                            .foregroundStyle(theme.ui(preferences.appearance == mode ? "text" : "secondary"))
                            .frame(maxWidth: .infinity).padding(.vertical, 11)
                            .background(RoundedRectangle(cornerRadius: 7).fill(theme.ui(preferences.appearance == mode ? "selection" : "editor")))
                            .overlay(RoundedRectangle(cornerRadius: 7).stroke(preferences.appearance == mode ? theme.ui("accent") : theme.ui("border"), lineWidth: 1))
                    }.buttonStyle(.plain)
                }
            }
            VStack(alignment: .leading, spacing: 9) {
                Text("PALETTA").font(.system(size: 10, weight: .semibold)).tracking(1).foregroundStyle(theme.ui("secondary"))
                LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 9) {
                    ForEach(ThemeCatalog.shared.palettes) { palette in
                        let sample = palette.resolve(dark: preferences.appearance.isDark(systemDark: systemScheme == .dark))
                        Button { preferences.paletteID = palette.id } label: {
                            VStack(alignment: .leading, spacing: 9) {
                                HStack(spacing: 5) { ForEach(["command", "environment", "math", "literal"], id: \.self) { role in Capsule().fill(sample.ui(role)).frame(width: 22, height: 4) }; Spacer() }
                                HStack { Text(palette.name).font(.system(size: 12, weight: .medium)); Spacer(); if preferences.paletteID == palette.id { Image(systemName: "checkmark.circle.fill").foregroundStyle(sample.ui("accent")) } }
                            }.foregroundStyle(sample.ui("text")).padding(12).frame(maxWidth: .infinity)
                                .background(RoundedRectangle(cornerRadius: 7).fill(sample.ui("editor")))
                                .overlay(RoundedRectangle(cornerRadius: 7).stroke(preferences.paletteID == palette.id ? theme.ui("accent") : theme.ui("border"), lineWidth: preferences.paletteID == palette.id ? 1.5 : 1))
                        }.buttonStyle(.plain).help(palette.description)
                    }
                }
            }
            Divider().overlay(theme.ui("border"))
            HStack {
                Text("Szerkesztő betűmérete").font(.system(size: 12))
                Spacer()
                IDEIconButton(symbol: "minus", label: "Kisebb betű") { preferences.fontSize = max(11, preferences.fontSize - 1) }
                Text("\(Int(preferences.fontSize))").font(.system(size: 12, design: .monospaced)).frame(width: 22)
                IDEIconButton(symbol: "plus", label: "Nagyobb betű") { preferences.fontSize = min(22, preferences.fontSize + 1) }
            }
            HStack(spacing: 8) { Circle().fill(theme.ui("command")).frame(width: 5, height: 5); Text("LaTeX-szintaxiskiemelés"); Spacer(); Image(systemName: "checkmark").foregroundStyle(theme.ui("success")) }.font(.system(size: 11)).foregroundStyle(theme.ui("secondary"))
            Toggle("LaTeX-parancskiegészítés", isOn: $preferences.completionEnabled).font(.system(size: 11)).toggleStyle(.switch).controlSize(.small)
        }.foregroundStyle(theme.ui("text")).padding(20).frame(width: 350).background(theme.ui("sidebar"))
    }
}
