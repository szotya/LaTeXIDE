#ifndef LATEXIDE_CORE_H
#define LATEXIDE_CORE_H

#ifdef __cplusplus
extern "C" {
#endif

#define LATEXIDE_ABI_VERSION 1
typedef struct LatexideRuntime LatexideRuntime;

/* Inputs are UTF-8, NUL-terminated strings. Calls on a handle must finish before freeing it. */
LatexideRuntime *latexide_runtime_new(const char *project_root);
void latexide_runtime_free(LatexideRuntime *runtime);
/* Returns an owned UTF-8 JSON reply. Free only with latexide_string_free. */
char *latexide_dispatch(LatexideRuntime *runtime, const char *request_json);
void latexide_string_free(char *reply);

#ifdef __cplusplus
}
#endif
#endif
