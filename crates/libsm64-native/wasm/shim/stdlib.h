#pragma once
#include <stddef.h>
void *malloc(size_t); void *calloc(size_t, size_t); void *realloc(void *, size_t); void free(void *);
void abort(void) __attribute__((noreturn)); void exit(int) __attribute__((noreturn));
int abs(int); long labs(long); int atoi(const char *); char *getenv(const char *);
void qsort(void *, size_t, size_t, int (*)(const void *, const void *));
#define EXIT_FAILURE 1
#define EXIT_SUCCESS 0
