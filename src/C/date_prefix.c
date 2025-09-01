#include <stdio.h>
#include <time.h>

const char* get_datetime_str_c() {
    static char buffer[32];
    time_t now;
    struct tm *t;

    time(&now);
    t = localtime(&now);

    strftime(buffer, sizeof(buffer), "%Y.%m.%d %H:%M:%S", t);

    return buffer;
}

const char* get_date_str_c() {
    static char buffer[32];
    time_t now;
    struct tm *t;

    time(&now);
    t = localtime(&now);

    strftime(buffer, sizeof(buffer), "%Y.%m.%d", t);

    return buffer;
}