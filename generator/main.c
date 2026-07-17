
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <unistd.h>

#include "libsais64.h"

#if defined(__clang__)
// Shut up clangd.
extern int ftruncate(int, int);
#endif

typedef uint8_t u8;
typedef uint64_t u64;
typedef int64_t i64;

#define arralloc(T, n) ((T *)malloc((n) * sizeof(T)))
#define handle_error(msg) \
    do { perror((msg)); exit(EXIT_FAILURE); } while (0)

void generate_from_file(int argc, char *argv[]);

int main(int argc, char *argv[]) {
    generate_from_file(argc, argv);
    return 0;
}

void generate_from_file(int argc, char *argv[]) {
    if (argc < 4) {
        fprintf(stderr, "program <INPUT_PATH> <BWT_OUTPUT_PATH> <LCP_OUTPUT_PATH>\n");
        return;
    }

    int status;
    char *input_file_path = argv[1];
    char *bwt_path = argv[2];
    char *lcp_path = argv[3];
    struct stat file_information = {0};

    int input_fd = open(input_file_path, O_RDONLY);
    if (input_fd < 0) handle_error("open input_file_path");

    status = fstat(input_fd, &file_information);
    if (status < 0) handle_error("fstat");

    i64 total_length = file_information.st_size;
    i64 len = total_length - 1;
    u8 *string = (u8 *)mmap(NULL, total_length, PROT_READ, MAP_PRIVATE, input_fd, 0);
    if (string == NULL) handle_error("mmap input_fd");

    int bwt_fd = open(bwt_path, O_RDWR | O_CREAT, 0644);
    if (bwt_fd < 0) handle_error("open bwt_path");
    status = ftruncate(bwt_fd, len);
    if (status < 0) handle_error("ftruncate bwt_fd"); 
    u8 *bwt = (u8 *)mmap(NULL, len, PROT_READ | PROT_WRITE, MAP_SHARED, bwt_fd, 0);
    if (bwt == NULL) handle_error("mmap bwt_fd");

    int lcp_fd = open(lcp_path, O_RDWR | O_CREAT, 0644);
    if (lcp_fd < 0) handle_error("open lcp_path");
    i64 lcp_len = len * sizeof(i64);
    status = ftruncate(lcp_fd, lcp_len);
    if (status < 0) handle_error("ftruncate lcp_fd"); 
    i64 *lcp = (i64 *)mmap(NULL, lcp_len, PROT_READ | PROT_WRITE, MAP_SHARED, lcp_fd, 0);
    if (lcp == NULL) handle_error("mmap lcp_fd");
    
    i64 freq[256];

    // Use the lcp file mapped memory for the temporary array.
    i64 primary = libsais64_bwt(string, bwt, lcp, len, 0, freq);
    if (primary < 0) {
        printf("bwt error\n");
    }
    printf("bwt primary: [%ld]\n", primary);

    u8 chars[] = {'$', 'A', 'C', 'G', 'T'};
    printf("(%2d) 0: %ld\n", 0, freq[0]);
    for (int i = 0; i < 5; ++i) {
        printf("(%2d) %c: %ld\n", chars[i], chars[i], freq[chars[i]]);
    }
    munmap(bwt, len);

    u8 *shifted_string = string + 1;
    i64 *sa = arralloc(i64, len);
    i64 err = libsais64(shifted_string, sa, len, 0, NULL);
    if (err < 0) {
        printf("sa error\n");
        free(sa);
        munmap(string, total_length);
        munmap(lcp, lcp_len);
        return;
    }

    i64 *plcp = arralloc(i64, total_length);
    err = libsais64_plcp(shifted_string, sa, plcp, len);
    munmap(string, total_length);
    if (err < 0) {
        printf("plcp error\n");
        free(sa);
        free(plcp);
        munmap(lcp, lcp_len);
        return;
    }

    err = libsais64_lcp(plcp, sa, lcp, len);
    free(sa);
    free(plcp);
    munmap(lcp, lcp_len);
    if (err < 0) {
        printf("lcp error\n");
    }
}

