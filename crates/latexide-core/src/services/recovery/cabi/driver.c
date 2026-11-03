/* A real C caller: only public v1 opaque handles and matching string frees.
 * No recovery-only runtime, exported test hooks or protocol changes. */
#include "latexide_core.h"
#include <errno.h>
#include <fcntl.h>
#include <sandbox.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static void deadline(int sig) { (void)sig; _exit(124); }
static void output(LatexideRuntime *runtime, const char *request) {
    char *reply = latexide_dispatch(runtime, request);
    if (!reply) _exit(71);
    puts(reply);
    fflush(stdout);
    latexide_string_free(reply);
}

int main(int argc, char **argv) {
    if (argc != 3 || strchr(argv[2], '"') || strchr(argv[2], '\\')) return 64;
    signal(SIGALRM, deadline);
    alarm(9); /* The helper always exits itself; the parent never kills it. */
    char profile[8192], *error = NULL;
    int n = snprintf(profile, sizeof profile,
        "(version 1)(allow default)(deny file-write*)(deny network*)"
        "(deny process-fork)(deny process-exec)"
        "(allow file-write* (subpath \"%s\")"
        " (literal \"/private/tmp/latexide-%u\")"
        " (regex #\"^/private/tmp/latexide-%u/%d-[0-9a-f]+[.](json|sock)$\"))"
        "(allow network-inbound (local unix-socket))"
        "(allow network-outbound (remote unix-socket))",
        argv[2], (unsigned)geteuid(), (unsigned)geteuid(), getpid());
    if (n < 0 || (size_t)n >= sizeof profile || sandbox_init(profile, 0, &error)) {
        fprintf(stderr, "SANDBOX_BLOCKED: %s\n", error ? error : "profile length");
        if (error) sandbox_free_error(error);
        return 77;
    }
    char denied[4096];
    snprintf(denied, sizeof denied, "%s-denied-probe-%d", argv[2], getpid());
    errno = 0;
    int fd = open(denied, O_WRONLY|O_CREAT|O_EXCL, 0600);
    if (fd != -1 || (errno != EACCES && errno != EPERM)) {
        if (fd != -1) { close(fd); unlink(denied); }
        fputs("SANDBOX_GUARD_FAILED\n", stderr);
        return 77;
    }
    fprintf(stderr, "SANDBOX_ACTIVE own_pid=%d write_denial=%d deadline_seconds=9\n", getpid(), errno);
    LatexideRuntime *runtime = latexide_runtime_new(argv[1]);
    if (!runtime) { puts("{\"factory_rejected\":true}"); return 65; }
    output(runtime, "{\"op\":\"workspace_info\"}");
    char *line = NULL; size_t capacity = 0; ssize_t length;
    while ((length = getline(&line, &capacity, stdin)) != -1) {
        if (length > 1024*1024+1) { free(line); return 66; }
        if (!strcmp(line, "!exit\n")) _exit(73);
        if (!strcmp(line, "!drop_reply\n")) {
            /* No synthetic ACK: next real C result is freed without delivery. */
            if (getline(&line, &capacity, stdin) == -1) return 67;
            char *reply = latexide_dispatch(runtime, line);
            if (!reply) return 71;
            latexide_string_free(reply);
            _exit(73);
        }
        output(runtime, line);
    }
    free(line);
    latexide_runtime_free(runtime);
    latexide_runtime_free(NULL);
    latexide_string_free(NULL);
    fputs("TEARDOWN_COMPLETE\n", stderr);
    return 0;
}
