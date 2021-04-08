#include <stdio.h>
#include <assert.h>
#include <stdlib.h>
#include <unistd.h>

int main(void) {
    printf("Enter something:\n");
    printf("> ");
    fflush(stdout);

    char buf[3];
    assert(buf != NULL);
    int nread = read(STDIN_FILENO, buf, sizeof(buf) - 1);
    if (nread < 0)
        exit(EXIT_FAILURE);
    printf("nread: %d\n", nread);
    buf[nread] = 0;
    printf("\"%s\"\n", buf);
}
