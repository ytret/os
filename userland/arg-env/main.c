#include <stdio.h>
#include <stdint.h>

int main(int argc, char **argv, char **environ) {
    printf("argc = %d\n", argc);

    for (int i = 0; i < argc; i++) {
        printf("argv[%d] = %s\n", i, argv[i]);
    }

    if (!environ) {
        printf("environ = NULL\n");
    } else {
        printf("environ = 0x%08X\n", (uint32_t) environ);
        int i;
        for (i = 0; environ[i] != NULL; i++) {
            printf("environ[%d] = %s\n", i, environ[i]);
        }
        printf("environ[%d] = NULL\n", i);
    }

    fflush(stdout);
    for (;;);

    return 0;
}
