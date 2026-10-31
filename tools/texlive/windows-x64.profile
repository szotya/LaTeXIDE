# Exact x64 portable bootstrap profile. Adjust all C:/LateX_IDE paths together
# to the Windows project directory before execution. Not executed on macOS.
# After bootstrap, use tlmgr --verify-repo=all install for package-roots.json.
selected_scheme scheme-infraonly
TEXDIR C:/LateX_IDE/.local/texlive-windows-x64
TEXMFLOCAL C:/LateX_IDE/.local/texlive-windows-x64/texmf-local
TEXMFSYSCONFIG C:/LateX_IDE/.local/texlive-windows-x64/texmf-config
TEXMFSYSVAR C:/LateX_IDE/.local/texlive-windows-x64/texmf-var
TEXMFCONFIG C:/LateX_IDE/.local/texlive-windows-x64/texmf-config
TEXMFVAR C:/LateX_IDE/.local/texlive-windows-x64/texmf-var
TEXMFHOME C:/LateX_IDE/.local/texlive-windows-x64/texmf-local
binary_windows 1
instopt_adjustpath 0
instopt_portable 1
instopt_letter 0
instopt_write18_restricted 0
tlpdbopt_create_formats 1
tlpdbopt_desktop_integration 0
tlpdbopt_file_assocs 0
tlpdbopt_install_docfiles 0
tlpdbopt_install_srcfiles 0
tlpdbopt_post_code 1
tlpdbopt_w32_multi_user 0
