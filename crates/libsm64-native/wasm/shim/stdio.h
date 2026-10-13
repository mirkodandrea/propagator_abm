#pragma once
#include <stddef.h>
#include <stdarg.h>
typedef struct _FILE FILE;
extern FILE *stdout, *stderr, *stdin;
int printf(const char *, ...); int fprintf(FILE *, const char *, ...);
int sprintf(char *, const char *, ...); int snprintf(char *, size_t, const char *, ...);
int vsnprintf(char *, size_t, const char *, va_list); int vfprintf(FILE *, const char *, va_list);
int puts(const char *); int putchar(int); int fflush(FILE *);
FILE *fopen(const char *, const char *); int fclose(FILE *);
size_t fread(void *, size_t, size_t, FILE *); size_t fwrite(const void *, size_t, size_t, FILE *);
int fseek(FILE *, long, int); long ftell(FILE *);
#define SEEK_SET 0
#define SEEK_END 2
#define SEEK_CUR 1
#define EOF (-1)
int feof(FILE *); int ferror(FILE *); int getc(FILE *); int ungetc(int, FILE *); int fgetc(FILE *); int fputc(int, FILE *); int fputs(const char *, FILE *);
char *fgets(char *, int, FILE *); int sscanf(const char *, const char *, ...); int fscanf(FILE *, const char *, ...);
