#pragma once
double sqrt(double); double pow(double, double); double ldexp(double, int); double floor(double); double ceil(double);
double fabs(double); double sin(double); double cos(double); double atan2(double, double); double log(double); double exp(double); double fmod(double, double);
float sqrtf(float); float sinf(float); float cosf(float); float atan2f(float, float); float fabsf(float);
float floorf(float); float ceilf(float); float powf(float, float); float tanf(float); float acosf(float); float asinf(float);
float fmodf(float, float); float roundf(float); float atanf(float); float logf(float); float expf(float);
double round(double); double trunc(double); float truncf(float);
#define M_PI 3.14159265358979323846
#define INFINITY (__builtin_inff())
#define NAN (__builtin_nanf(""))
#define HUGE_VALF (__builtin_inff())
#define isnan(x) __builtin_isnan(x)
#define isinf(x) __builtin_isinf(x)
double frexp(double, int *); double modf(double, double *); double fmax(double, double); double fmin(double, double);
