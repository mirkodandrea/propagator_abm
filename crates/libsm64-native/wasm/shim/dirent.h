#pragma once
#include <stddef.h>
typedef struct DIR DIR;
struct dirent { char d_name[256]; };
DIR *opendir(const char *); struct dirent *readdir(DIR *); int closedir(DIR *);
