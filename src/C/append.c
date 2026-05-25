#include <stdio.h>

static FILE* safe_fopen(const char *filename, const char *mode) {
#if defined(_WIN32) || defined(_WIN64)
    FILE *file = NULL;
    fopen_s(&file, filename, mode);
    return file;
#else
    return fopen(filename, mode);
#endif
}

int append_to_file_c(const char *filename, const char *string_to_append) {
    FILE *file = safe_fopen(filename, "a");
    if (file == NULL) {
        return 1;
    }

    if (fputs(string_to_append, file) == EOF) {
        fclose(file);
        return 2;
    }

    if (putc('\n', file) == EOF) {
        fclose(file);
        return 3;
    }

    fclose(file);
    return 0;
}
