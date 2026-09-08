#include <stdio.h>
#include <pwd.h>
int main(void) {
    struct passwd *p = getpwnam("podboxsupplied");
    printf("SUPPLIED_PASSWD=%s\n", p ? "seen" : "not-seen");
    return 0;
}
