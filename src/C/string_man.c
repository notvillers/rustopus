#include <stddef.h>
#include <stdlib.h>
#include <string.h>

char *remove_breaks(const char *str) {
    size_t len = strlen(str);
    char *result = (char *)malloc(len + 1);
    if (!result) return (char *)str;
    memcpy(result, str, len + 1);

    for (size_t i = 0; i < len; i++) {
        if (result[i] == '\n' || result[i] == '\r') {
            result[i] = ' ';
        }
    }

    return result;
}
