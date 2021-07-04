#include <stdio.h>
#include <unistd.h>

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    if (fork() == 0) {
        printf("Child\n");
    } else {
        printf("Parent\n");
    }
    printf("PID: %d\n", getpid());
}
