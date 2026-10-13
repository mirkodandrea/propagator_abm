// The few libc functions libsm64 links against, for wasm32-unknown-unknown
// (no libc there). Memory comes from Rust (wasm.rs); there is no filesystem,
// so file functions fail and printing is dropped. `mem*` come from
// compiler_builtins.
#include <stddef.h>
#include <stdarg.h>
#include <stdint.h>
#include <string.h>
#include <stdio.h>
#include <math.h>
#include <stdlib.h>

int g_verbosity = 0;
FILE *stdout = 0, *stderr = 0, *stdin = 0;

float sqrtf(float x) { return __builtin_sqrtf(x); }
int abs(int x) { return x < 0 ? -x : x; }
int strcmp(const char *a, const char *b) {
    while (*a && *a == *b) { a++; b++; }
    return (unsigned char)*a - (unsigned char)*b;
}
double frexp(double x, int *e) {
    union { double d; uint64_t u; } v = { x };
    int ex = (int)((v.u >> 52) & 0x7ff);
    if (ex == 0 || ex == 0x7ff) { *e = 0; return x; }
    *e = ex - 1022;
    v.u = (v.u & ~(0x7ffULL << 52)) | (1022ULL << 52);
    return v.d;
}
float read_f32_be(unsigned char *b) {
    union { uint32_t i; float f; } r;
    r.i = ((uint32_t)b[0] << 24) | ((uint32_t)b[1] << 16) | ((uint32_t)b[2] << 8) | b[3];
    return r.f;
}

FILE *fopen(const char *p, const char *m) { (void)p; (void)m; return 0; }
int fclose(FILE *f) { (void)f; return -1; }
size_t fread(void *p, size_t s, size_t n, FILE *f) { (void)p; (void)s; (void)n; (void)f; return 0; }
size_t fwrite(const void *p, size_t s, size_t n, FILE *f) { (void)p; (void)s; (void)n; (void)f; return 0; }
int fseek(FILE *f, long o, int w) { (void)f; (void)o; (void)w; return -1; }
long ftell(FILE *f) { (void)f; return -1; }
int feof(FILE *f) { (void)f; return 1; }
int printf(const char *f, ...) { (void)f; return 0; }
int fprintf(FILE *o, const char *f, ...) { (void)o; (void)f; return 0; }
int sprintf(char *b, const char *f, ...) { (void)f; b[0] = 0; return 0; }
