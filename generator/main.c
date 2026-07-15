
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "libsais.h"

typedef uint8_t u8;
typedef uint64_t u64;
typedef int32_t i32;

#define arralloc(T, n) ((T *)malloc((n) * sizeof(T)))

void tiny_test();

int main() {
    tiny_test();
    return 0;
}

void tiny_test() {
    const u8 *example = (u8 *)"$CCACC$GGA$GCAT$GCA$";

    i32 len = strlen((char *)example);

    u8 *string = arralloc(u8, len);
    memcpy(string, example, len);

    i32 *sa = arralloc(i32, len);
    i32 *plcp = arralloc(i32, len);
    i32 *lcp = arralloc(i32, len);
    u8 *bwt = arralloc(u8, len);
    i32 *temp = arralloc(i32, len);
    i32 freq[256];

    i32 err = libsais(string, sa, len, 0, NULL);
    if (err < 0) {
        printf("sa error\n");
        goto defer;
    }

    err = libsais_plcp(string, sa, plcp, len);
    if (err < 0) {
        printf("plcp error\n");
        goto defer;
    }

    err = libsais_lcp(plcp, sa, lcp, len);
    if (err < 0) {
        printf("lcp error\n");
        goto defer;
    }
 
    i32 primary = libsais_bwt(string, bwt, temp, len, 0, freq);
    if (primary < 0) {
        printf("bwt error\n");
        goto defer;
    }

    printf("%d\n", primary);
    printf("%.*s\n", len, bwt);

    {
        printf("%2d", sa[0]);
        for (int i = 1; i < len; ++i) {
            printf(", %2d", sa[i]);
        }
        printf("\n");
    }
    {
        printf("%2d", lcp[0]);
        for (int i = 1; i < len; ++i) {
            printf(", %2d", lcp[i]);
        }
        printf("\n");
    }

    u8 chars[] = {'$', 'A', 'C', 'G', 'T'};
    for (int i = 0; i < 5; ++i) {
        printf("(%2d) %c: %d\n", chars[i], chars[i], freq[chars[i]]);
    }

defer:
    free(temp);
    free(bwt);
    free(sa);
    free(plcp);
    free(lcp);
    free(string);
    return;
}

