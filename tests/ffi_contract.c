#include "latexide_core.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

static void check(LatexideRuntime *runtime, const char *request, const char *expected) {
    char *reply = latexide_dispatch(runtime, request);
    assert(reply != NULL);
    if (strstr(reply, expected) == NULL) {
        fprintf(stderr, "Unexpected FFI reply: %s\n", reply);
        assert(0);
    }
    latexide_string_free(reply);
}

int main(int argc, char **argv) {
    assert(argc == 2);
    LatexideRuntime *runtime = latexide_runtime_new(argv[1]);
    assert(runtime != NULL);
    check(runtime, "{", "invalid_request");
    check(runtime, "{\"op\":\"open_document\",\"path\":\"main.tex\"}", "\"ok\":true");
    check(runtime, "{\"op\":\"apply_edits\",\"document_id\":\"main.tex\",\"expected_revision\":1,\"edits\":[{\"start\":{\"line\":0,\"character\":1},\"end\":{\"line\":0,\"character\":1},\"text\":\"Agent\"}]}", "\"revision\":2");
    check(runtime, "{\"op\":\"save_document\",\"document_id\":\"main.tex\",\"expected_revision\":2}", "\"dirty\":false");
    check(runtime, "{\"op\":\"read_document\",\"document_id\":\"main.tex\"}", "aAgentbc");
    check(runtime, "{\"op\":\"undo\"}", "\"revision\":3");
    check(runtime, "{\"op\":\"read_document\",\"document_id\":\"main.tex\"}", "\"text\":\"abc\"");
    check(runtime, "{\"op\":\"redo\"}", "\"revision\":4");
    latexide_runtime_free(runtime);
    latexide_runtime_free(NULL);
    latexide_string_free(NULL);
    puts("PASS: C ABI lifecycle, malformed input, direct edit, save, undo and redo");
    return 0;
}
