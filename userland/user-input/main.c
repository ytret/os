#include <stdio.h>
#include <assert.h>
#include <stdlib.h>
#include <unistd.h>
#include <errno.h>

int main(void) {
    printf("Enter something:\n");
    printf("> ");
    fflush(stdout);

    char buf[3];
    int nread = read(STDIN_FILENO, buf, sizeof(buf) - 1);
    if (nread <= 0) {
        perror("read");
        exit(EXIT_FAILURE);
    }

    printf("nread: %d\n", nread);
    buf[nread] = 0;
    printf("buf: \"%s\"\n", buf);
}
