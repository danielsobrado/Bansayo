#include "ahrs.h"
#include <math.h>
#include <string.h>

static const float stationary_gyro_rad_s = 0.12f;
static const float stationary_accel_tolerance = 0.08f;
static const float correction_accel_tolerance = 0.20f;
static const float max_step_s = 0.1f;

static float norm3(const float v[3])
{
    return sqrtf(v[0]*v[0] + v[1]*v[1] + v[2]*v[2]);
}

static bool normalize(float q[4])
{
    const float n = sqrtf(q[0]*q[0] + q[1]*q[1] + q[2]*q[2] + q[3]*q[3]);
    if (!isfinite(n) || n < 1e-6f) return false;
    for (unsigned i = 0; i < 4; ++i) q[i] /= n;
    return true;
}

bool ahrs_stationary(const float gyro[3], const float accel[3])
{
    return norm3(gyro) < stationary_gyro_rad_s &&
           fabsf(norm3(accel) / BIMU_GRAVITY_M_S2 - 1.0f) < stationary_accel_tolerance;
}

bool ahrs_init(ahrs_t *s, const float a[3], const float bias[3], float gain)
{
    const float n = norm3(a);
    if (!isfinite(n) || n < 1e-6f || !isfinite(norm3(bias)) ||
        !isfinite(gain) || gain < 0) return false;
    ahrs_t next = {.gain = gain};
    memcpy(next.bias, bias, sizeof(next.bias));
    /* Shortest rotation from measured gravity-up to reference +Y. */
    next.q[0] = -a[2] / n;
    next.q[2] = a[0] / n;
    next.q[3] = 1.0f + a[1] / n;
    if (next.q[3] < 1e-6f && hypotf(next.q[0], next.q[2]) < 1e-6f) {
        next.q[0] = 1;
        next.q[2] = next.q[3] = 0;
    }
    if (!normalize(next.q)) return false;
    *s = next;
    return true;
}

bool ahrs_update(ahrs_t *s, const float gyro[3], const float a[3], float dt)
{
    if (!isfinite(dt) || dt <= 0 || dt > max_step_s || !isfinite(norm3(gyro))) return false;
    const float x = s->q[0], y = s->q[1], z = s->q[2], w = s->q[3];
    float g[3] = {gyro[0]-s->bias[0], gyro[1]-s->bias[1], gyro[2]-s->bias[2]};
    const float n = norm3(a);
    if (isfinite(n) && fabsf(n/BIMU_GRAVITY_M_S2 - 1) < correction_accel_tolerance) {
        const float up[3] = {2*(x*y+w*z), 1-2*(x*x+z*z), 2*(y*z-w*x)};
        const float u[3] = {a[0]/n, a[1]/n, a[2]/n};
        g[0] += s->gain*(u[1]*up[2]-u[2]*up[1]);
        g[1] += s->gain*(u[2]*up[0]-u[0]*up[2]);
        g[2] += s->gain*(u[0]*up[1]-u[1]*up[0]);
    }
    const float h = dt*0.5f;
    float q[4] = {
        x+h*(w*g[0]+y*g[2]-z*g[1]),
        y+h*(w*g[1]+z*g[0]-x*g[2]),
        z+h*(w*g[2]+x*g[1]-y*g[0]),
        w-h*(x*g[0]+y*g[1]+z*g[2])
    };
    if (!normalize(q)) return false;
    memcpy(s->q, q, sizeof(q));
    return true;
}
