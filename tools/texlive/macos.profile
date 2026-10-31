# Template: install_local.py resolves @TEXLIVE_PREFIX@ for this checkout.
# Portable infrastructure first; targeted packages are installed by tlmgr.
selected_scheme scheme-infraonly
TEXDIR @TEXLIVE_PREFIX@
TEXMFLOCAL @TEXLIVE_PREFIX@/texmf-local
TEXMFSYSCONFIG @TEXLIVE_PREFIX@/texmf-config
TEXMFSYSVAR @TEXLIVE_PREFIX@/texmf-var
TEXMFCONFIG @TEXLIVE_PREFIX@/texmf-config
TEXMFVAR @TEXLIVE_PREFIX@/texmf-var
TEXMFHOME @TEXLIVE_PREFIX@/texmf-local
binary_universal-darwin 1
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
