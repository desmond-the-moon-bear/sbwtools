
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <time.h>
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

void timestamp(const char* message);
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

    timestamp("[generate_from_file] begin");

    i64 status;
    char *input_file_path = argv[1];
    char *bwt_path = argv[2];
    char *lcp_path = argv[3];
    struct stat file_information = {0};

    int input_fd = open(input_file_path, O_RDONLY);
    if (input_fd < 0) handle_error("open input_file_path");

    status = fstat(input_fd, &file_information);
    if (status < 0) handle_error("fstat");

    i64 total_length = file_information.st_size;
    // i64 len = total_length - 1;
    i64 len = total_length;
    u8 *string = (u8 *)mmap(NULL, total_length, PROT_READ, MAP_PRIVATE, input_fd, 0);
    close(input_fd);
    if (string == MAP_FAILED) handle_error("mmap input_fd");

    int bwt_fd = open(bwt_path, O_RDWR | O_CREAT | O_TRUNC, 0644);
    if (bwt_fd < 0) handle_error("open bwt_path");
    u8 *bwt = arralloc(u8, len);
    if (bwt == NULL) handle_error("malloc bwt");

    int lcp_fd = open(lcp_path, O_RDWR | O_CREAT | O_TRUNC, 0644);
    if (lcp_fd < 0) handle_error("open lcp_path");
    i64 lcp_buffer_len = len * sizeof(i64);
    i64 *lcp = arralloc(i64, len);
    if (lcp == NULL) handle_error("malloc lcp");
    
    i64 freq[256];

    // Use the lcp file mapped memory for the temporary array.
    timestamp("[generate_from_file] bwt");
    i64 primary = libsais64_bwt(string, bwt, lcp, len, 0, freq);
    if (primary < 0) {
        printf("bwt error\n");
    }

    printf("bwt primary: [%ld]\n", primary);
    u8 chars[] = {'$', 'A', 'C', 'G', 'T', '#'};
    for (int i = 0; i < 6; ++i) {
        printf("(%2d) %c: %ld\n", chars[i], chars[i], freq[chars[i]]);
    }

    i64 offset = 0;
    while (offset < len) {
        status = write(bwt_fd, bwt + offset, len - offset);
        if (status <= 0) {
            break;
        }
        offset += status;
    }
    if (status < 0) handle_error("write bwt");
    free(bwt);
    if (close(bwt_fd) < 0) handle_error("close bwt_fd");

    timestamp("[generate_from_file] suffix array");
    status = libsais64(string, lcp, len, 0, NULL);
    if (status < 0) {
        printf("sa error\n");
        munmap(string, total_length);
        free(lcp);
        return;
    }

    timestamp("[generate_from_file] plcp");
    i64 *plcp = arralloc(i64, total_length);
    status = libsais64_plcp(string, lcp, plcp, len);
    if (status < 0) {
        printf("plcp error\n");
        free(plcp);
        munmap(string, total_length);
        free(lcp);
        return;
    }

    timestamp("[generate_from_file] lcp");
    status = libsais64_lcp(plcp, lcp, lcp, len);
    free(plcp);
    munmap(string, total_length);

    offset = 0;
    u8 *lcp_bytes = (u8 *)lcp;
    while (offset < lcp_buffer_len) {
        status = write(lcp_fd, lcp_bytes + offset, lcp_buffer_len - offset);
        if (status <= 0) {
            break;
        }
        offset += status;
    }
    if (status < 0) handle_error("write lcp");
    free(lcp);
    if (close(lcp_fd) < 0) handle_error("close lcp_fd");

    if (status < 0) {
        printf("lcp error\n");
    }

    timestamp("[generate_from_file] done");
}

void timestamp(const char* message) {
    time_t raw_time;
    struct tm *time_info;
    time(&raw_time);
    time_info = localtime(&raw_time);
    char *time_string = asctime(time_info);
    int len = strlen(time_string);
    printf("[%.*s] %s\n", len-1, time_string, message);
    fflush(stdout);
}

