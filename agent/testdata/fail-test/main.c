#include <stdio.h>
int quench_keep_me(void) { return 42; }
int main(void) {
    printf("elapsed_ms=1.0\n");
    printf("marker=%d\n", quench_keep_me());
    return 0;
}
