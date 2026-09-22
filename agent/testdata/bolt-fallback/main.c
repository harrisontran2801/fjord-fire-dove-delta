#include <stdio.h>
int main(void) {
    volatile long s = 0;
    for (int i = 0; i < 10000; i++) {
        s += i;
    }
    printf("ok %ld\n", s);
    return 0;
}
