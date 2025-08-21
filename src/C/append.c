#include <stdio.h>

int append_to_file_c(const char *filename, const char *string_to_append) {
    FILE *file = fopen(filename, "a");
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
