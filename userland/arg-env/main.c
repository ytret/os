#include <stdio.h>
#include <stdint.h>

int main(int argc, char **argv, char **environ) {
    printf("argc = %d\n", argc);

    int i;
    for (i = 0; i < argc; i++) {
        printf("argv[%d] = %s\n", i, argv[i]);
    }
    printf("argv[%d] = NULL\n", i);

    if (!environ) {
        printf("environ = NULL\n");
    } else {
        printf("environ = 0x%08X\n", (uint32_t) environ);
        int j;
        for (j = 0; environ[j] != NULL; j++) {
            printf("environ[%d] = %s\n", j, environ[j]);
        }
        printf("environ[%d] = NULL\n", j);
    }

    fflush(stdout);
    for (;;);

    return 0;
}
