#include <stdio.h>
#include <time.h>

static void safe_localtime(const time_t *now, struct tm *out) {
#ifdef _WIN32
    localtime_s(out, now);
#else
    localtime_r(now, out);
#endif
}

const char* get_datetime_str_c() {
    static char buffer[32];
    time_t now;
    struct tm t;

    time(&now);
    safe_localtime(&now, &t);
    strftime(buffer, sizeof(buffer), "%Y.%m.%d %H:%M:%S", &t);

    return buffer;
}

const char* get_date_str_c() {
    static char buffer[32];
    time_t now;
    struct tm t;

    time(&now);
    safe_localtime(&now, &t);
    strftime(buffer, sizeof(buffer), "%Y.%m.%d", &t);

    return buffer;
}
